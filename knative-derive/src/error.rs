use crate::REQUIRED_VARIANTS;
use proc_macro2::Span;
use syn::Error;
use std::fmt;

pub enum VerificationError {
    NotDependent(String),
    OneRequiredVariant,
}

impl fmt::Display for VerificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use VerificationError::*;
        match self {
            NotDependent(s) => f.write_fmt(format_args!("{} may not be a dependent", s)),
            OneRequiredVariant => f.write_fmt(
                format_args!(
                    "ConditionType must contain only one of either {} variant",
                    REQUIRED_VARIANTS.join(" or ")
                )
            )
        }
    }
}

impl From<VerificationError> for Error {
    fn from(v: VerificationError) -> Error {
        Error::new(Span::call_site(), v)
    }
}
