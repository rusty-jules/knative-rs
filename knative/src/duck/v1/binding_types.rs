use k8s_openapi::{
    api::core::v1::ObjectReference,
    apimachinery::pkg::apis::meta::v1::{LabelSelector, ObjectMeta},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Binding {
    kind: String,
    api_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<ObjectMeta>,
    spec: BindingSpec,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug, JsonSchema)]
pub struct BindingSpec {
    // We diverge from knative go for the binding spec.
    // The Binding relies heavily of ducktyping, as described
    // in the docs: https://knative.dev/docs/reference/concepts/duck-typing/#binding
    pub subject: Reference,
}

// Found in knative.dev/pkg/tracker
// TODO: implement From<Resource> for Reference
/// Reference is modeled after corev1.ObjectReference, but omits fields
/// unsupported by the tracker, and permits us to extend things in
/// divergent ways.
#[derive(Serialize, Deserialize, Clone, Default, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Reference {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(flatten)]
    pub subject: Subject
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Subject {
    Name(String),
    Selector(LabelSelector)
}

impl Default for Subject {
    fn default() -> Self {
        Subject::Name("".into())
    }
}

impl From<Reference> for ObjectReference {
    fn from(reference: Reference) -> ObjectReference {
        let Reference { api_version, kind, namespace, subject } = reference;
        ObjectReference {
            api_version,
            kind,
            namespace,
            name: match subject {
                Subject::Name(name) => Some(name),
                Subject::Selector(..) => None
            },
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn serialize_reference() {
        use serde_json::json;
        let reference = Reference {
            kind: Some("kind".into()),
            api_version: Some("dev.derevit/v1".into()),
            namespace: None,
            subject: Subject::Name("my-pod".into()),
        };
        let json = json!({
            "kind": "kind",
            "apiVersion": "dev.derevit/v1",
            "name": "my-pod"
        });
        assert_eq!(serde_json::to_string(&reference).unwrap(), serde_json::to_string(&json).unwrap())
    }
}
