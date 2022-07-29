mod error;
mod inner;

use proc_macro::TokenStream;
use syn::{
    parse_macro_input,
    DeriveInput,
};

const REQUIRED_VARIANTS: [&str; 2] = ["Ready", "Succeeded"];

/// Derive [`knative_conditions::ConditionType`] on your own `enum` to adhere to the Knative Source schema and condition
/// management.
///
/// Automatically implements [`Default`] on your type, which must be the top level condition.
///
/// # Example
/// ```rust
/// use knative_derive::ConditionType;
///
/// #[derive(ConditionType, Debug, Copy, Clone, PartialEq)]
/// enum MyCondition {
///   // First condition must be Ready or Succeeded
///   Ready,
///   // Use the dependent attribute to mark conditions
///   // that are required to consider the resource ready
///   #[dependent]
///   SinkProvided,
///   #[dependent]
///   Important,
///   // Conditions that are not marked dependent do not
///   // determine overall resource readiness
///   Informational,
/// }
/// ```
#[proc_macro_derive(ConditionType, attributes(dependent))]
pub fn derive(input: TokenStream) -> TokenStream {
    let ast: DeriveInput = parse_macro_input!(input);

    match inner::inner_derive(ast) {
        Ok(v) => v,
        Err(e) => e.to_compile_error().into()
    }
}

// Shout out to @johnhoo for his excellent proc macro tutorial!
// This probably would have been too scary to attempt without it:
// https://youtu.be/geovSK3wMB8
