#![allow(dead_code)]
use super::{
    knative_reference::KReference,
    status_types::{
        ConditionType, ConditionManager, Conditions, Condition, Status,
    },
};
use crate::error::{DiscoveryError, Error};
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(CustomResource, Serialize, Deserialize, Default, Clone, Debug, JsonSchema)]
#[kube(
    kind = "Source",
    group = "knative.dev",
    status = "SourceStatus",
    version = "v1",
    namespaced
)]
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
    /// URI can be an absolute URL(non-empty scheme and non-empty host) pointing to the target or a relative URI. Relative URIs will be resolved using the base URI retrieved from Ref.
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
                    (None, _) => None
                },
                group: None,
                kind: reference.kind,
                namespace: reference.namespace,
                name: reference.name
            }),
            uri: None
        }
    }
}

impl From<url::Url> for Destination {
    fn from(uri: url::Url) -> Self {
        Destination {
            ref_: None,
            uri: Some(uri)
        }
    }
}

impl Destination {
    pub fn resolve_uri(&self, client: kube::Client) -> Result<url::Url, Error> {
        match (&self.ref_, &self.uri) {
            (Some(ref ref_), _) => {
                let url = ref_.resolve_uri(client)?;
                Ok(url)
            }
            (None, Some(ref uri)) => Ok(uri.clone()),
            (None, None) => Err(Error::Discovery(DiscoveryError::EmptyDestination))
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

/// SourceStatus shows how we expect folks to embed Addressable in
/// their Status field.
#[derive(Deserialize, Serialize, Clone, Debug, Default, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SourceStatus {
    /// inherits Status, which currently provides:
    /// * ObservedGeneration - the 'Generation' of the Service that was last
    ///   processed by the controller.
    /// * Conditions - the latest available observations of a resource's current
    ///   state.
    #[serde(flatten)]
    pub status: Status,
    /// SinkURI is the current active sink URI that has been configured for the
    /// Source.
    pub sink_uri: Option<url::Url>,
    /// CloudEventAttributes are the specific attributes that the Source uses
    /// as part of its CloudEvents.
    pub cloud_event_attributes: Option<Vec<CloudEventAttributes>>,
}

/// Allows a source status CR to manage its Conditions
pub trait SourceManager<'de, const N: usize> {
    /// The [`ConditionTypes`] that the `Ready` status of the source depends on.
    ///
    /// [`ConditionTypes`]: ../status_types/enum.ConditionType.html
    type Dependents: Deserialize<'de> + Serialize + Clone + JsonSchema + ToString;

    /// Return the dependent ConditionTypes. Can be defined in a number of ways:
    /// ### As &'static str
    ///
    /// ```rust
    /// impl<'de> SourceManager<'de, 1> for MyStatus {
    ///     type Dependents = &'static str;
    ///
    ///     fn dependents() -> [Self::Dependents; 1] {
    ///         ["SinkProvided"]
    ///     }
    ///
    ///     ...
    /// }
    /// ```
    ///
    /// ### As an Enum
    ///
    /// ```rust
    /// use schemars::JsonSchema;
    /// use serde::{Serialize, Deserialize};
    /// use std::fmt;
    ///
    /// #[derive(Deserialize, Serialize, Clone, JsonSchema)]
    /// enum MyConditions {
    ///     SinkProvided,
    ///     VeryImportant,
    ///     NotImportant
    /// }
    ///
    /// impl fmt::Display for MyConditions {
    ///     fn fmt<'a>(&self, fmt: &mut fmt::Formatter<'a>) -> fmt::Result {
    ///         std::write!(fmt, "{:?}", self)
    ///     }
    /// }
    ///
    /// impl<'de> SourceManager<'de, 2> for MyStatus {
    ///     type Dependents = MyConditions;
    ///
    ///     fn dependents() -> [Self::Dependents; 2] {
    ///         [MyConditions::SinkProvided, MyConditions::VeryImportant]
    ///     }
    ///
    ///     ...
    /// }
    /// ```
    fn dependents() -> [Self::Dependents; N];

    /// Return the conditions of your CRD Status type.
    fn conditions(&mut self) -> &mut Conditions;

    /// Return the SourceStatus of your CRD Status type.
    fn source_status(&mut self) -> &mut SourceStatus;

    /// Construct a [`ConditionManager`] of your dependent Conditions and the
    /// `Ready` or `Succeeded` status.
    ///
    /// [`ConditionManager`]: ../status_types/struct.ConditionManager.html
    fn manager(&mut self) -> ConditionManager<N> {
        ConditionManager::new_living(
            Self::dependents().map(|d| ConditionType::Extension(d.to_string())),
            self.conditions()
        )
    }

    /// Returns true if the resource is ready overall.
    fn is_ready(&mut self) -> bool {
        self.manager().is_happy()
    }

    /// Set the condition that the source has a sink configured
    fn mark_sink(&mut self, uri: url::Url) {
        self.source_status().sink_uri = Some(uri);
        self.manager().mark_true(&ConditionType::sinkprovided());
    }

    /// Set the condition that the source has no sink configured
    fn mark_no_sink(&mut self, reason: &str, message: Option<String>) {
        self.source_status().sink_uri = None;
        self.manager().mark_false(&ConditionType::sinkprovided(), reason, message);
    }

    /// Set the condition that the source status is unknown. Typically used when beginning the
    /// reconciliation of a new generation.
    fn mark_unknown(&mut self) {
        let mut cm = self.manager();
        let type_ = &cm.get_top_level_condition().type_.clone();
        cm.mark_unknown(
            type_,
            "NewObservedGenFailure",
            Some("unsuccessfully observed a new generation".into())
        );
    }
}

#[allow(unreachable_patterns)]
impl<'de> SourceManager<'de, 1> for SourceStatus {
    type Dependents = ConditionType;

    fn dependents() -> [ConditionType; 1] {
        [ConditionType::sinkprovided()]
    }

    fn conditions(&mut self) -> &mut Conditions {
        match self.status.conditions {
            Some(ref mut conditions) => conditions,
            None => {
                self.status.conditions = Some(Conditions::with_conditions(
                    vec![
                        Condition::default(),
                        Condition {
                            type_: ConditionType::sinkprovided(),
                            ..Default::default()
                        },
                    ]));
                self.conditions()
            }
        }
    }

    fn source_status(&mut self) -> &mut SourceStatus {
        self
    }
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
