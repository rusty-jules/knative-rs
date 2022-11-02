use k8s_openapi::api::core::v1::Service;
use kube::Config;
use kube::api::DynamicObject;
use kube::api::{Resource, ResourceExt};
use thiserror::Error;
use url::Url;
use serde_json::Value;
use serde::Deserialize;

#[derive(Error, Debug)]
pub enum AddressableErr {
    #[error("{0} ({1}) is not an AddressableType")]
    NotAddressable(String, String),
    #[error("url missing in address of {0}")]
    UrlNotSet(String),
    #[error("service must have name to be addressable")]
    ServiceMustHaveName,
    #[error("service must have namespace")]
    ServiceMustHaveNamespace,
    #[error("unable to infer Kubeconfig: {0}")]
    InferConfigErr(#[from] kube::config::InferConfigError),
    #[error("unable to find Kubeconfig: {0}")]
    KubeconfigErr(#[from] kube::config::KubeconfigError),
    #[error("unable to parse url: {0}")]
    UrlParseErr(#[from] url::ParseError)
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

#[doc(hidden)]
/// Construct the uri from the service metadata
async fn build_service_url(name: &str, namespace: &str) -> Result<Url, AddressableErr> {
    let cluster_url = Config::infer().await?.cluster_url;
    let scheme = cluster_url.scheme().unwrap_or(&http::uri::Scheme::HTTP);
    let cluster_host = cluster_url.host().unwrap_or("cluster.local");
    let url = Url::parse(&format!("{scheme}://{name}.{namespace}.svc.{cluster_host}"))?;

    Ok(url)
}

#[doc(hidden)]
/// Parse a url from a &serde_json::Value containing a status. This avoids a clone of data.
fn parse_url_from_obj_data(name: &str, kind: &str, data: &Value) -> Result<Url, AddressableErr> {
    if let Some(data) = data.as_object() {
        if let Some(status) = data.get("status").and_then(Value::as_object) {
            if let Some(address) = status.get("address").and_then(Value::as_object) {
                return match address.get("url").and_then(Value::as_str).map(Url::parse) {
                    Some(url) => Ok(url?),
                    None => Err(AddressableErr::UrlNotSet(name.to_string()))
                }
            }
        }
    }
    Err(AddressableErr::NotAddressable(name.to_string(), kind.to_string()))
}

#[async_trait::async_trait]
pub trait AddressableTypeExt {
    async fn try_get_address(&self) -> Result<Url, AddressableErr>;
}

#[async_trait::async_trait]
impl AddressableTypeExt for AddressableType {
    async fn try_get_address(&self) -> Result<Url, AddressableErr> {
        self.status.address.url
            .clone()
            .ok_or_else(|| AddressableErr::UrlNotSet("addressable".to_string()))
    }
}

#[async_trait::async_trait]
impl AddressableTypeExt for DynamicObject {
    async fn try_get_address(&self) -> Result<Url, AddressableErr> {
        let name = self.meta().name.as_ref().ok_or(AddressableErr::ServiceMustHaveName)?;
        let namespace = self.namespace().unwrap_or_else(|| "default".into());

        match &self.types {
            Some(t) => match (t.api_version.as_ref(), t.kind.as_ref()) {
                ("v1", "Service") => build_service_url(name, &namespace).await,
                _ => parse_url_from_obj_data(name, t.kind.as_ref(), &self.data)
            }
            None => Err(AddressableErr::NotAddressable(name.to_string(), "unknown".to_string()))
        }
    }
}

#[async_trait::async_trait]
impl AddressableTypeExt for Service {
    async fn try_get_address(&self) -> Result<Url, AddressableErr> {
        let name = self.meta().name.as_ref().ok_or(AddressableErr::ServiceMustHaveName)?;
        let namespace = self.namespace().unwrap_or_else(|| "default".into());
        build_service_url(name, &namespace).await
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use serde::de::DeserializeOwned;
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

    fn read_mock<T: Resource + DeserializeOwned>(filename: &str) -> T {
        let path = mock_path() + filename;
        let yaml = fs::read_to_string(path).expect("path to mock");
        serde_yaml::from_str(&yaml).unwrap()
    }

    #[async_std::test]
    async fn broker_uri() {
        let broker = read_mock::<DynamicObject>("default_broker.yaml");
        let uri = broker.try_get_address().await.expect("broker is addressable");
        assert_eq!(uri.scheme(), "http");
        assert_eq!(uri.host().unwrap().to_string(), "broker-ingress.default.svc.cluster.local");
        assert_eq!(uri.path(), "/default/default");
    }

    #[async_std::test]
    async fn broker_status_deserializes_into_addressable() {
        let broker = read_mock::<DynamicObject>("default_broker.yaml");
        let addressable: AddressableType = serde_json::from_value(broker.data)
            .expect("broker status deserializes into AddressableType");
        let uri = addressable.try_get_address().await
            .expect("url set on default broker");
        assert_eq!(uri.scheme(), "http");
        assert_eq!(uri.host().unwrap().to_string(), "broker-ingress.default.svc.cluster.local");
        assert_eq!(uri.path(), "/default/default");
    }

    #[async_std::test]
    async fn service_uri() {
        setup_kubeconfig();
        let service = read_mock::<DynamicObject>("default_service.yaml");
        let uri = service.try_get_address().await.expect("to read config");
        assert_eq!(uri.scheme(), "http");
        assert_eq!(uri.host().unwrap().to_string(), "default.default.svc.cluster.local");
        assert_eq!(uri.path(), "/");
    }

    #[async_std::test]
    async fn service_struct_uri() {
        setup_kubeconfig();
        let service = read_mock::<Service>("default_service.yaml");
        let uri = service.try_get_address().await.expect("");
        assert_eq!(uri.scheme(), "http");
        assert_eq!(uri.host().unwrap().to_string(), "default.default.svc.cluster.local");
        assert_eq!(uri.path(), "/");
    }
}

