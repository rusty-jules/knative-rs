use crate::error::Error;
use k8s_openapi::api::core::v1::ObjectReference;
use kube::{
    api::{DynamicObject, GroupVersionKind},
    core::object,
    discovery, Api, ResourceExt,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// KReference contains enough information to refer to another object.
/// It's a trimmed down version of corev1.ObjectReference.
#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KReference {
    /// Kind of the referent.
    /// More info: https://git.k8s.io/community/contributors/devel/sig-architecture/api-conventions.md#types-kinds
    pub kind: String,
    /// Namespace of the referent.
    /// More info: https://kubernetes.io/docs/concepts/overview/working-with-objects/namespaces/
    /// This is optional field, it gets defaulted to the object holding it if left out.
    /// Note: This API is EXPERIMENTAL and might break anytime. For more details: https://github.com/knative/eventing/issues/5086
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    /// Name of the referent.
    /// More info: https://kubernetes.io/docs/concepts/overview/working-with-objects/names/#names
    pub name: String,
    /// API version of the referent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_version: Option<String>,
    /// Group of the API, without the version of the group. This can be used as an alternative to the APIVersion, and then resolved using ResolveGroup.
    /// Note: This API is EXPERIMENTAL and might break anytime. For more details: https://github.com/knative/eventing/issues/5086
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

impl Into<ObjectReference> for KReference {
    fn into(self) -> ObjectReference {
        ObjectReference {
            name: Some(self.name),
            namespace: self.namespace,
            api_version: self.api_version,
            kind: Some(self.kind),
            ..Default::default()
        }
    }
}

impl KReference {
    pub async fn resolve_uri(
        &self,
        client: kube::Client,
    ) -> Result<url::Url, Box<dyn std::error::Error>> {
        // let object_reference: ObjectReference = self.clone().into();
        let gvk = GroupVersionKind::gvk(
            self.group.as_ref().unwrap(),
            self.api_version.as_ref().unwrap(),
            self.kind.as_ref(),
        );
        let (ar, _caps) = discovery::pinned_kind(&client, &gvk).await?;
        let api: Api<DynamicObject> = Api::all_with(client.clone(), &ar);
        let _obj = api.get(self.name.as_ref()).await?;
        // TODO: into duckv1.AddressableType is not implemented yet.
        unimplemented!("see knative.dev/pkg/resolver/addressable_resolver.go")
    }
}
