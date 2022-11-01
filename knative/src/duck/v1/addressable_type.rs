use crate::error::Error;

use kube::Config;
use kube::api::DynamicObject;
use thiserror::Error;
use url::Url;
use serde::Deserialize;

#[derive(Error, Debug)]
pub enum AddressableErr {
    #[error("{0} ({1}) is not an AddressableType")]
    NotAddressable(String, String),
    #[error("URL missing in address of {0}")]
    UrlNotSet(String),
    #[error("Service must have name to be addressable")]
    ServiceMustHaveName,
    #[error("Service must have namespace")]
    ServiceMustHaveNamespace,
    #[error("Unable to find Kubeconfig: {0}")]
    KubeconfigErr(#[from] kube::config::KubeconfigError)
}

impl AddressableErr {
    fn not_addressable(DynamicObject { metadata, types, .. }: DynamicObject) -> Self {
        Self::NotAddressable(
            metadata.name.as_ref().map(|n| n.clone()).unwrap_or_else(|| "".to_string()),
            types.as_ref().map(|t| t.kind.clone()).unwrap_or_else(|| "unknown".to_string())
        )
    }
}

#[derive(Deserialize)]
pub struct Addressable {
    pub url: Option<Url>
}

#[derive(Deserialize)]
pub struct AddressableStatus {
    pub address: Addressable,
}

#[derive(Deserialize)]
pub struct AddressableType {
    pub status: AddressableStatus
}

impl AddressableType {
    pub fn is_addressable(obj: DynamicObject) -> bool {
        match obj.types {
            Some(t) => match (t.api_version.as_ref(), t.kind.as_ref()) {
                ("v1", "Service") => true,
                _ => serde_json::from_value::<AddressableType>(obj.data).is_ok()
            }
            None => false
        }
    }

    pub async fn try_get_uri(obj: DynamicObject) -> Result<url::Url, Error> {
        let name = obj.metadata.name.unwrap_or_else(|| "".into());
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
                            .map_err(|_| AddressableErr::NotAddressable(name.clone(), t.kind.clone()))?;
                        Ok(addressable.status.address.url.ok_or(AddressableErr::UrlNotSet(name))?)
                    }
                }
            }
            None => {
                Err(AddressableErr::NotAddressable(name, "unknown".into()))?
            }
        }
    }
}

fn extract_url_from_value(v: &serde_json::Value) -> Option<Url> {
    if let Some(data) = v.as_object() {
        if data.contains_key("status") {
            if let Some(status) = data["status"].as_object() {
                if status.contains_key("address") {
                    if let Some(address) = status["address"].as_object() {
                        if address.contains_key("url") && address["url"].as_str().is_some() {
                            return match address["url"].as_str().map(Url::parse) {
                                Some(Ok(url)) => Some(url),
                                None | Some(Err(_)) => None
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

impl TryInto<AddressableType> for DynamicObject {
    type Error = AddressableErr;
    fn try_into(self) -> Result<AddressableType, Self::Error> {
        let name = self.metadata.name.as_ref().ok_or(AddressableErr::ServiceMustHaveName)?;
        let namespace = self.metadata.namespace.as_ref().ok_or(AddressableErr::ServiceMustHaveNamespace)?;
        match &self.types {
            Some(t) => match (t.api_version.as_ref(), t.kind.as_ref()) {
                ("v1", "Service") => {
                    let cluster_url_host = {
                        // Copied straight from kube_client::config::file_loader to avoid async
                        // params
                        let config = kube::config::Kubeconfig::read()?;
                        let context_name = match &config.current_context {
                            Some(name) => name,
                            None => Err(kube::config::KubeconfigError::CurrentContextNotSet)?
                        };
                        let current_context = config
                            .contexts
                            .iter()
                            .find(|named_context| &named_context.name == context_name)
                            .map(|named_context| &named_context.context)
                            .ok_or_else(|| kube::config::KubeconfigError::LoadContext(context_name.clone()))?;
                        let cluster_name = &current_context.cluster;
                        let cluster = config
                            .clusters
                            .iter()
                            .find(|named_cluster| &named_cluster.name == cluster_name)
                            .map(|named_cluster| &named_cluster.cluster)
                            .ok_or_else(|| kube::config::KubeconfigError::LoadClusterOfContext(cluster_name.clone()))?;
                        let cluster_url = cluster
                            .server
                            .parse::<http::Uri>()
                            .map_err(kube::config::KubeconfigError::ParseClusterUrl)?;
                        cluster_url
                    };
                    // Construct the uri from the service metadata
                    let url = Url::parse(
                        &format!("http://{name}.{namespace}.svc.{cluster_url_host}")
                    ).expect("valid url from service and config");
                    Ok(AddressableType {
                        status: AddressableStatus {
                            address: Addressable {
                                url: Some(url)
                            }
                        }
                    })
                }
                _ => serde_json::from_value::<AddressableType>(self.data)
                        .map_err(|_| AddressableErr::NotAddressable(name.clone(), t.kind.clone()))
            }
            None => Err(AddressableErr::not_addressable(self))
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::fs;

    fn mock_path() -> String {
        format!("{}/{}/",
            env!("CARGO_MANIFEST_DIR"),
            "../test/mock",
        )
    }

    fn setup_kubeconfig() {
        std::env::set_var("KUBECONFIG", mock_path() + "kubeconfig.yaml");
    }

    fn read_mock(filename: &str) -> DynamicObject {
        let path = mock_path() + filename;
        let yaml = fs::read_to_string(path).expect("path to mock");
        serde_yaml::from_str(&yaml).unwrap()
    }

    #[test]
    fn broker_is_addressable() {
        let broker = read_mock("default_broker.yaml");
        assert!(AddressableType::is_addressable(broker));
    }

    #[test]
    fn service_is_addressable() {
        let service = read_mock("default_service.yaml");
        assert!(AddressableType::is_addressable(service));
    }

    #[async_std::test]
    async fn broker_uri() {
        setup_kubeconfig();
        let broker = read_mock("default_broker.yaml");
        let uri = AddressableType::try_get_uri(broker).await.expect("broker is addressable");
        assert_eq!(uri.scheme(), "http");
        assert_eq!(uri.host().unwrap().to_string(), "broker-ingress.default.svc.cluster.local");
        assert_eq!(uri.path(), "/default/default");
    }

    #[async_std::test]
    async fn service_uri() {
        setup_kubeconfig();
        let service = read_mock("default_service.yaml");
        let uri = AddressableType::try_get_uri(service).await.expect("to read config");
        assert_eq!(uri.scheme(), "http");
        assert_eq!(uri.host().unwrap().to_string(), "default.default.svc.cluster.local");
        assert_eq!(uri.path(), "/");
    }
}

