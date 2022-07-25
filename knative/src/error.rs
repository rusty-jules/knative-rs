use thiserror::Error;
use kube::error::Error as KubeError;

#[derive(Error, Debug)]
pub enum Error {
    /// Discovery errors
    #[error("Error from discovery: {0}")]
    Discovery(#[source] DiscoveryError),
    /// Kube errors
    #[error("Error: {0}")]
    KubeError(#[from] KubeError),
}

#[derive(Error, Debug, Clone, Copy)]
pub enum DiscoveryError {
    #[error("destination missing Ref and URI, expected at least one")]
    EmptyDestination,
    #[error("resolve kreference is not implemented")]
    ResolveKReferenceNotImplemented
}
