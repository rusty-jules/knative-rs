use kube::CustomResource;
use knative::duck::v1 as duckv1;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// SinkBinding describes a Binding that is also a Source.
/// The `sink` is resolved to a URL and then projected into
/// the `subject` by augmenting the definition of the
/// referenced containers to have a `K_SINK` environment
/// variable holding the endpoint to which to send cloud events.
#[derive(CustomResource, Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[kube(
    kind = "SinkBinding",
    group = "sources.knative.dev",
    status = "SinkBindingStatus",
    version = "v1alpha2",
    namespaced
)]
#[serde(rename_all = "camelCase")]
pub struct SinkBindingSpec {
    /// Sink and CloudEventOverrides
    #[serde(flatten)]
    pub source_spec: duckv1::source_types::SourceSpec,
    /// Subject
    #[serde(flatten)]
    pub binding_spec: duckv1::binding_types::BindingSpec,
}

/// SinkBindingStatus communicates the observed state of the SinkBinding (from the controller).
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
pub struct SinkBindingStatus {
    /// inherits SourceStatus, which currently provides:
    /// * observed_generation
    /// * conditions
    /// * sink_uri
    #[serde(flatten)]
    pub source_status: duckv1::source_types::SourceStatus<duckv1::source_types::SourceCondition, 1>,
}
