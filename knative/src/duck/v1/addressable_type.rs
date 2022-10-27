use crate::error::Error;

use kube::Config;
use kube::api::DynamicObject;
use thiserror::Error;
use url::Url;
use serde::Deserialize;

use std::borrow::Cow;

#[derive(Error, Debug, Clone)]
pub enum AddressableErr {
    #[error("{0} ({1}) is not an AddressableType")]
    NotAddressable(String, String),
    #[error("URL missing in address of {0}")]
    UrlNotSet(String)
}

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
                        Ok(addressable.status.address.url
                            .into_owned()
                            .ok_or(AddressableErr::UrlNotSet(name))?)
                    }
                }
            }
            None => {
                Err(AddressableErr::NotAddressable(name, "unknown".into()))?
            }
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

