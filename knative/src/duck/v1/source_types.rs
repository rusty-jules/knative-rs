#![allow(dead_code)]
use super::{
    knative_reference::KReference,
    status_types::Status,
};
use crate::derive::ConditionType;
use crate::error::{DiscoveryError, Error};
use knative_conditions::{ConditionAccessor, Conditions};
use enumset::EnumSetType;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Clone, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SourceSpec {
    /// Sink is a reference to an object that will resolve to a uri to use as the sink.
    pub sink: Option<Destination>,
    // CloudEventOverrides defines overrides to control the output format and
    // modifications of the event sent to the sink.
    pub ce_overrides: Option<CloudEventOverrides>,
}

impl SourceSpec {
    pub fn ce_overrides(&self) -> Option<CloudEventOverrides> {
        self.ce_overrides.clone()
    }
}

/// Destination represents a target of an invocation over HTTP.
#[derive(Deserialize, Serialize, Clone, Debug, Default, JsonSchema)]
pub struct Destination {
    /// Ref points to an Addressable.
    #[serde(rename = "ref")]
    ref_: Option<KReference>,
    /// URI can be an absolute URL(non-empty scheme and non-empty host) pointing to the target or a relative URI.
    /// Relative URIs will be resolved using the base URI retrieved from Ref.
    pub uri: Option<url::Url>,
}

impl From<KReference> for Destination {
    fn from(reference: KReference) -> Self {
        Destination {
            ref_: Some(KReference {
                // combine the group and api_version, handling the case that this was done already
                api_version: match (reference.api_version, reference.group) {
                    (Some(api_version), _) if api_version.contains("/") => Some(api_version),
                    (Some(api_version), Some(group)) => Some(group + "/" + &api_version),
                    (Some(api_version), None) => Some(api_version),
                    (None, _) => None,
                },
                group: None,
                kind: reference.kind,
                namespace: reference.namespace,
                name: reference.name,
            }),
            uri: None,
        }
    }
}

impl From<url::Url> for Destination {
    fn from(uri: url::Url) -> Self {
        Destination {
            ref_: None,
            uri: Some(uri),
        }
    }
}

impl Destination {
    pub async fn resolve_uri(
        &self,
        client: kube::Client,
    ) -> Result<url::Url, Error> {
        match (&self.ref_, &self.uri) {
            (Some(ref ref_), _) => {
                let url = ref_.resolve_uri(client).await?;
                Ok(url)
            }
            (None, Some(ref uri)) => Ok(uri.clone()),
            (None, None) => Err(Error::Discovery(DiscoveryError::EmptyDestination)),
        }
    }
}

/// CloudEventOverrides defines arguments for a Source that control the output
/// format of the CloudEvents produced by the Source.
#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CloudEventOverrides {
    /// Extensions specify what attribute are added or overridden on the
    /// outbound event. Each `Extensions` key-value pair are set on the event as
    /// an attribute extension independently.
    pub extensions: Option<std::collections::BTreeMap<String, String>>,
}

/// CloudEventAttributes specifies the attributes that a Source
/// uses as part of its CloudEvents.
#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CloudEventAttributes {
    #[serde(rename = "type")]
    type_: Option<String>,
    source: Option<String>,
}

/// A baseline [`ConditionType`] for [`SourceStatus`].
///
/// Custom conditions should implement [`SourceConditionType`] in order to be used by
/// [`SourceStatus`].
#[derive(ConditionType, EnumSetType, Deserialize, Serialize, Debug, JsonSchema)]
pub enum SourceCondition {
    Ready,
    /// A [`sink_uri`] has been set on the resource.
    ///
    /// [`sink_uri`]:./struct.SourceStatus.html#structfield.sink_uri
    #[dependent]
    SinkProvided
}

/// SourceStatus shows how we expect folks to embed Addressable in
/// their Status field.
#[derive(Deserialize, Serialize, Clone, Debug, Default, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SourceStatus<S: SourceConditionType> {
    /// inherits Status, which currently provides:
    /// * ObservedGeneration - the 'Generation' of the Service that was last
    ///   processed by the controller.
    /// * Conditions - the latest available observations of a resource's current
    ///   state.
    #[serde(flatten)]
    pub status: Status<S>,
    /// SinkURI is the current active sink URI that has been configured for the
    /// Source.
    pub sink_uri: Option<url::Url>,
    /// CloudEventAttributes are the specific attributes that the Source uses
    /// as part of its CloudEvents.
    pub cloud_event_attributes: Option<Vec<CloudEventAttributes>>,
}

impl<S: SourceConditionType> ConditionAccessor<S> for SourceStatus<S> {
    fn conditions(&mut self) -> &mut Conditions<S> {
        self.status.conditions()
    }
}

/// Provides management `sink_uri` on [`SourceStatus`].
///
/// This traits helps to discourage use of the `*sinkprovided()` methods from
/// [`SourceConditionManager`], which must be disambiguated when using a custom [`ConditionType`]
/// that also has `*sinkprovided()` methods.
pub trait SinkManager<S: SourceConditionType>: SourceConditionManager<S> {
    /// Return the [`SourceStatus`] of your CRD Status type.
    fn source_status(&mut self) -> &mut SourceStatus<S>;

    /// Set the condition that the source has a sink configured
    fn mark_sink(&mut self, uri: url::Url) {
        self.source_status().sink_uri = Some(uri);
        self.manager().mark_true(S::sinkprovided());
    }

    /// Set the condition that the source has no sink configured
    fn mark_no_sink(&mut self, reason: &str, message: Option<String>) {
        self.source_status().sink_uri = None;
        self.manager().mark_false(S::sinkprovided(), reason, message);
    }
}

impl<S: SourceConditionType> SinkManager<S> for SourceStatus<S> {
    fn source_status(&mut self) -> &mut SourceStatus<S> {
        self
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::derive::ConditionType;

    struct MyStatus {
        source_status: SourceStatus<SourceCondition>
    }

    #[test]
    fn can_manage_sink() {
        let mut status = MyStatus {
            source_status: SourceStatus::default()
        };
        status.source_status.mark_sink("http://url".parse().unwrap());
    }

    #[derive(ConditionType, EnumSetType, Debug)]
    enum MyCondition {
        Ready,
        #[dependent]
        SinkProvided,
        #[dependent]
        Important,
        Unimportant
    }

    impl SourceConditionType for MyCondition {
        fn sinkprovided() -> Self {
            MyCondition::SinkProvided
        }
    }

    struct MyCustomStatus {
        source_status: SourceStatus<MyCondition>
    }

    #[test]
    fn can_manage_sink_on_source_status() {
        let mut status = MyCustomStatus {
            source_status: SourceStatus::default()
        };
        let uri = "http://url".parse::<url::Url>().unwrap();
        status.source_status.mark_sink(uri.clone());
        assert_eq!(status.source_status.sink_uri, Some(uri));
        assert_eq!(status.manager().get_condition(MyCondition::SinkProvided).map(|c| c.is_true()), Some(true))
    }

    #[test]
    fn all_conditions_determine_ready() {
        let mut status = MyCustomStatus {
            source_status: SourceStatus::default()
        };
        let s = &mut status.source_status;

        assert_eq!(s.is_ready(), false);

        s.mark_unimportant();
        assert_eq!(s.is_ready(), false);

        s.mark_sink("http://url".parse().unwrap());
        assert_eq!(s.is_ready(), false);

        s.mark_important();
        assert_eq!(s.is_ready(), true);

        s.mark_not_important("ImportantReason", None);
        assert_eq!(s.is_ready(), false);

        s.mark_important();
        assert_eq!(s.is_ready(), true);

        s.mark_no_sink("NotSink", None);
        assert_eq!(s.is_ready(), false);

        s.mark_sink("http://url".parse().unwrap());
        s.mark_important();
        s.mark_not_unimportant("NotImportant", None);
        assert_eq!(s.is_ready(), true);

        s.mark_unknown();
        assert_eq!(s.is_ready(), false);
    }

    #[test]
    fn can_manage_custom_conditions() {
        let mut status = MyCustomStatus {
            source_status: SourceStatus::default()
        };
        let s = &mut status.source_status;

        // Using MyConditionManager methods yields the same result as ConditionManager methods
        s.mark_important();
        s.mark_unimportant_with_reason(
            "NotImportant",
            Some("More information on Unimportant".into())
        );
        let old_conditions = s.conditions().clone();

        s.manager().mark_true(MyCondition::Important);
        s.manager()
            .mark_true_with_reason(
                MyCondition::Unimportant,
                "NotImportant",
                Some("More information on Unimportant".into())
        );
        assert_eq!(old_conditions, *s.conditions());
        assert_eq!(s.is_ready(), false);

        // Use of this function is discouraged because it does not guarantee that the sink has been
        // set on SourceStatus, but you may choose to handle sink_uri yourself.
        MyConditionManager::mark_sinkprovided(s);
        assert_eq!(s.is_ready(), true);
    }

    impl ConditionAccessor<MyCondition> for MyCustomStatus {
        fn conditions(&mut self) -> &mut Conditions<MyCondition> {
            self.source_status.conditions()
        }
    }

    impl SinkManager<MyCondition> for MyCustomStatus {
        fn source_status(&mut self) -> &mut SourceStatus<MyCondition> {
            &mut self.source_status
        }
    }

    #[test]
    fn can_manage_sink_on_custom_status() {
        let mut status = MyCustomStatus {
            source_status: SourceStatus::default()
        };
        let uri = "http://url".parse::<url::Url>().unwrap();
        status.mark_sink(uri.clone());
        assert_eq!(status.source_status.sink_uri, Some(uri));
        assert_eq!(status.manager().get_condition(MyCondition::SinkProvided).map(|c| c.is_true()), Some(true))
    }
}
