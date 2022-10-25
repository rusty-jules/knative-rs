use thiserror::Error;
use kube::error::Error as KubeError;

#[derive(Error, Debug)]
pub enum Error {
    /// Discovery errors
    #[error("Error from discovery: {0}")]
    Discovery(#[from] DiscoveryError),
    /// Kube errors
    #[error("Error: {0}")]
    KubeError(#[from] KubeError),
}

#[derive(Error, Debug, Clone)]
pub enum DiscoveryError {
    #[error("destination missing Ref and URI, expected at least one")]
    EmptyDestination,
    #[error("resolve kreference is not implemented")]
    ResolveKReferenceNotImplemented,
    #[error("{0} ({1}) is not an AddressableType")]
    NotAddressableType(String, String),
    #[error("URL missing in address of {0}")]
    UrlNotSetOnAddressable(String),
}
