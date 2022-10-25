use crate::error::DiscoveryError;

use kube::Config;
use kube::api::DynamicObject;
use url::Url;
use serde::Deserialize;

use std::borrow::Cow;

#[derive(Deserialize)]
pub struct Addressable<'a> {
    pub url: Cow<'a, Option<Url>>,
}

#[derive(Deserialize)]
pub struct AddressableStatus<'a> {
    pub address: Addressable<'a>,
}

#[derive(Deserialize)]
pub struct AddressableType<'a> {
    pub status: AddressableStatus<'a>
}

impl<'a> AddressableType<'a> {
    pub fn is_addressable(obj: DynamicObject) -> bool {
        match obj.types {
            Some(t) => match (t.api_version.as_ref(), t.kind.as_ref()) {
                ("v1", "Service") => true,
                _ => match serde_json::from_value::<AddressableType>(obj.data) {
                    Ok(_) => true,
                    Err(_) => false
                }
            }
            None => false
        }
    }

    pub async fn try_get_uri(obj: DynamicObject) -> Result<url::Url, DiscoveryError> {
        let name = obj.metadata.name.unwrap_or("".into());
        let namespace = obj.metadata.namespace.as_ref().unwrap();

        match obj.types {
            Some(t) => {
                match (t.api_version.as_ref(), t.kind.as_ref()) {
                    // K8s Services are special cased. They can be called
                    // even though they do not satisfy the Callable interface.
                    ("v1", "Service") => {
                        // Get the cluster host from the current kube config
                        let kube_config = Config::infer()
                            .await
                            .expect("kube config to be found");
                        // Construct the uri from the service metadata
                        let url = Url::parse(
                            &format!("http://{}.{}.svc.{}",
                                &name,
                                namespace,
                                kube_config.cluster_url.host().unwrap())
                        ).expect("valid url from service and config");
                        Ok(url)
                    }
                    _ => {
                        // The type must contain the fields on an Addressable
                        let addressable: AddressableType = serde_json::from_value(obj.data)
                            .map_err(|_| DiscoveryError::NotAddressableType(name.clone(), t.kind.clone()))?;
                        addressable.status.address.url
                            .into_owned()
                            .ok_or(DiscoveryError::UrlNotSetOnAddressable(name))
                    }
                }
            }
            None => {
                Err(DiscoveryError::NotAddressableType(name, "unknown".into()))
            }
        }
    }
}
