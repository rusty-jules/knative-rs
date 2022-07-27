#![allow(dead_code)]
use super::{
    knative_reference::KReference,
    status_types::Status,
};
use knative_conditions::{ConditionManager, Condition, Conditions};
use crate::error::{DiscoveryError, Error};
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
pub struct SourceStatus<S: SourceConditionType<N>, const N: usize> {
    /// inherits Status, which currently provides:
    /// * ObservedGeneration - the 'Generation' of the Service that was last
    ///   processed by the controller.
    /// * Conditions - the latest available observations of a resource's current
    ///   state.
    #[serde(flatten)]
    pub status: Status<S, N>,
    /// SinkURI is the current active sink URI that has been configured for the
    /// Source.
    pub sink_uri: Option<url::Url>,
    /// CloudEventAttributes are the specific attributes that the Source uses
    /// as part of its CloudEvents.
    pub cloud_event_attributes: Option<Vec<CloudEventAttributes>>,
}

/// A baseline ConditionType for SourceStatus.
/// Custom conditions should implement [`SourceConditionType`].
#[derive(crate::derive::ConditionType, Deserialize, Serialize, Copy, Clone, Debug, JsonSchema, PartialEq)]
pub enum SourceCondition {
    Ready,
    #[dependent]
    SinkProvided
}

pub trait SourceConditionType<const N:usize>: knative_conditions::ConditionType<N> {
    fn sinkprovided() -> Self;
}

impl SourceConditionType<1> for SourceCondition {
    fn sinkprovided() -> Self {
        Self::SinkProvided
    }
}

/// Allows a status to manage [`SourceConditionsType`].
pub trait SourceManager<S: knative_conditions::ConditionType<N> + SourceConditionType<N>, const N: usize> {
    /// Return the conditions of your CRD Status type.
    fn conditions(&mut self) -> &mut Conditions<S, N>;

    /// Return the [`SourceStatus`] of your CRD Status type.
    fn source_status(&mut self) -> &mut SourceStatus<S, N>;

    /// Construct a [`ConditionManager`] of your dependent Conditions and the
    /// `Ready` or `Succeeded` status.
    fn manager(&mut self) -> ConditionManager<S, N> {
        ConditionManager::new(self.conditions())
    }

    /// Returns true if the resource is ready overall.
    fn is_ready(&mut self) -> bool {
        self.manager().is_happy()
    }

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

    /// Set the condition that the source status is unknown. Typically used when beginning the
    /// reconciliation of a new generation.
    fn mark_unknown(&mut self) {
        let t = self.manager().get_top_level_condition().type_;
        self.manager().mark_unknown(
            t,
            "NewObservedGenFailure",
            Some("unsuccessfully observed a new generation".into())
        );
    }
}

impl SourceManager<SourceCondition, 1> for SourceStatus<SourceCondition, 1> {
    fn conditions(&mut self) -> &mut Conditions<SourceCondition, 1> {
        match self.status.conditions {
            Some(ref mut conditions) => conditions,
            None => {
                self.status.conditions = Some(Conditions::with_conditions(
                    vec![
                        Condition::default(),
                        Condition {
                            type_: SourceCondition::sinkprovided(),
                            ..Default::default()
                        },
                    ]));
                self.conditions()
            }
        }
    }

    fn source_status(&mut self) -> &mut SourceStatus<SourceCondition, 1> {
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
