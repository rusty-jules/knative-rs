use thiserror::Error;

#[derive(Error, Debug, Clone, Copy)]
pub enum Error {
    /// Discovery errors
    #[error("Error from discovery: {0}")]
    Discovery(#[source] DiscoveryError),
}

#[derive(Error, Debug, Clone, Copy)]
pub enum DiscoveryError {
    #[error("destination missing Ref and URI, expected at least one")]
    EmptyDestination,
    #[error("resolve kreference is not implemented")]
    ResolveKReferenceNotImplemented
}
