#![allow(dead_code)]
use super::{
    knative_reference::KReference,
    status_types::{ConditionStatus, ConditionType, Status},
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
    ) -> Result<url::Url, Box<dyn std::error::Error>> {
        match (&self.ref_, &self.uri) {
            (Some(ref ref_), _) => {
                let url = ref_.resolve_uri(client).await?;
                Ok(url)
            }
            (None, Some(ref uri)) => Ok(uri.clone()),
            (None, None) => Err(Box::new(Error::Discovery(DiscoveryError::EmptyDestination))),
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

#[allow(unreachable_patterns)]
impl SourceStatus {
    // returns true if the resource is ready overall.
    pub fn is_ready(&self) -> bool {
        match &self.status.conditions {
            Some(conditions) => conditions.iter().any(|c| match c.type_ {
                // Look for the "happy" condition, which is the only condition that
                // we can reliably understand to be the overall state of the resource.
                ConditionType::Ready | ConditionType::Succeeded => {
                    c.status == ConditionStatus::True
                }
                _ => false,
            }),
            None => false,
        }
    }

    pub fn mark_ready(&mut self) {
        if let Some(ref mut cond) = &mut self.status.conditions {
            cond.mark_true(ConditionType::Ready)
        }
    }

    /// Set the condition that the source has a sink configured
    pub fn mark_sink(&mut self, uri: url::Url) {
        self.sink_uri = Some(uri);
        if let Some(ref mut cond) = &mut self.status.conditions {
            cond.mark_true(ConditionType::SinkProvided)
        }
    }

    /// Set the condition that the source has no sink configured
    pub fn mark_no_sink(&mut self, reason: &str, message: String) {
        self.sink_uri = None;
        if let Some(ref mut cond) = &mut self.status.conditions {
            cond.mark_false(
                ConditionType::SinkProvided,
                reason.to_string(),
                Some(message),
            )
        }
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
