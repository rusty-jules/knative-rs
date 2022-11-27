use super::addressable_type::AddressableTypeExt;
use crate::error::Error;
use thiserror::Error;
use k8s_openapi::api::core::v1::ObjectReference;
use kube::{
    api::{DynamicObject, GroupVersionKind},
    discovery, Api,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Error, Clone, Copy)]
pub enum KRefErr {
    #[error("apiVersion is incomplete or group does not exist")]
    MalformedGVK,
    #[error("must be namespaced")]
    MustBeNamespaced,
}

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

impl From<KReference> for ObjectReference {
    fn from(reference: KReference) -> ObjectReference {
        ObjectReference {
            name: Some(reference.name),
            namespace: reference.namespace,
            api_version: reference.api_version,
            kind: Some(reference.kind),
            ..Default::default()
        }
    }
}

impl KReference {
    pub async fn resolve_uri(
        &self,
        client: kube::Client,
    ) -> Result<url::Url, Error> {
        let KReference {
            group,
            api_version,
            namespace,
            kind,
            name,
            ..
        } = self;

        let ns = namespace.as_ref()
            .ok_or(KRefErr::MustBeNamespaced)?;

        let (group, api_version) = match (group, api_version) {
            (Some(group), Some(api_version)) => {
                (group.as_str(), api_version.as_str())
            }
            (None, Some(api_version)) if api_version.contains('/') => {
                let mut iter = api_version.split('/');
                (iter.next().unwrap(), iter.next().unwrap())
            },
            _ => Err(KRefErr::MalformedGVK)?
        };

        let gvk = GroupVersionKind::gvk(
            group,
            api_version,
            kind,
        );

        let (ar, _caps) = discovery::pinned_kind(&client, &gvk).await?;
        let api = Api::<DynamicObject>::namespaced_with(client.clone(), ns, &ar);
        let obj = api.get(name).await?;
        let url = obj.address().await?;

        debug_assert!(!url.cannot_be_a_base());

        Ok(url)
    }
}
