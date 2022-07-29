#![doc = include_str!("../README.md")]
mod duck;
pub mod error;

// expose only v1 types
pub use duck::v1::*;

pub mod conditions {
    pub use knative_conditions::{ConditionAccessor, Conditions};
}

pub mod derive {
    pub use knative_derive::ConditionType;
}
