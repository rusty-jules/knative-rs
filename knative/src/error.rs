use crate::duck::v1::{
    addressable_type::AddressableErr,
    knative_reference::KRefErr,
    source_types::DestinationErr,
};
use thiserror::Error;
use kube::error::Error as KubeError;
use url::ParseError as UrlParseError;

#[derive(Error, Debug)]
pub enum Error {
    /// Kube errors
    #[error("Error: {0}")]
    KubeError(#[from] KubeError),
    /// Url errors
    #[error("Error invalid url: {0}")]
    UrlParseError(#[from] UrlParseError),
    /// Destination errors
    #[error("Error destination: {0}")]
    DestinationError(#[from] DestinationErr),
    /// KReference errors
    #[error("Error kreference: {0}")]
    KReferenceError(#[from] KRefErr),
    /// Addressable errors
    #[error("Error addressable: {0}")]
    AddressableError(#[from] AddressableErr)
}
