pub mod duck;
pub mod error;

pub mod conditions {
    pub use knative_conditions::{ConditionAccessor, Conditions};
}

pub mod derive {
    pub use knative_derive::ConditionType;
}

#[doc = include_str!("../../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
