use crate::duck::v1::knative_reference::KRefError;
use thiserror::Error;
use kube::error::Error as KubeError;
use url::ParseError as UrlParseError;

#[derive(Error, Debug)]
pub enum Error {
    /// Discovery errors
    #[error("Error from discovery: {0}")]
    Discovery(#[from] DiscoveryError),
    /// Kube errors
    #[error("Error: {0}")]
    KubeError(#[from] KubeError),
    /// Url errors
    #[error("Error invalid url: {0}")]
    UrlParseError(#[from] UrlParseError)
}

#[derive(Error, Debug, Clone)]
pub enum DiscoveryError {
    #[error("destination missing Ref and URI, expected at least one")]
    EmptyDestination,
    #[error("malformed kreference {0}")]
    KReference(#[from] KRefError),
    #[error("{0} ({1}) is not an AddressableType")]
    NotAddressableType(String, String),
    #[error("URL missing in address of {0}")]
    UrlNotSetOnAddressable(String),
}
