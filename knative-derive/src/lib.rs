use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input,
    Data::Enum,
    DeriveInput,
    Ident
};

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
    let ast = parse_macro_input!(input as DeriveInput);

    let name = &ast.ident;
    let variants = match ast.data {
        Enum(syn::DataEnum { ref variants, .. }) => variants,
        _ => panic!("ConditionType may only be derived on enums")
    };

    let required_variants = ["Ready", "Succeeded"];
    let dependents = variants.clone().into_iter()
        .filter(|v| v.attrs.iter().any(|a| a.path.segments.iter().any(|p| p.ident == "dependent")))
        .map(|v| v.ident);
    let dependents_again = dependents.clone();

    let variant_strings: Vec<String> = variants.iter().map(|v| v.ident.to_string()).collect();
    let dependent_strings: Vec<String> = dependents.clone().map(|d| d.to_string()).collect();

    if let Some(required) = dependent_strings.iter().find(|d| required_variants.contains(&d.as_str())) {
         panic!("{} variant may not be a dependent", required)
    }

    // check if both happy variants exist on the enum
    if required_variants.iter().all(|req| variant_strings.iter().any(|v| v == *req)) {
         panic!("ConditionType may only contain one of either Ready or Succeeded variants")
    }

    let happy = variants.iter().find(|v| required_variants.contains(&v.ident.to_string().as_str()))
            .expect("ConditionType must contain either Ready or Succeeded variant");
    let capitalized = variants.iter()
        .map(|v| v.ident.clone())
        .filter(|v| !required_variants.contains(&v.to_string().as_str()));
    let lower_case = capitalized.clone()
        .map(|v| Ident::new(&v.to_string().to_lowercase(), v.span()));
    let lower_case_doc = capitalized.clone().map(|c| format!("Returns the `{c}` variant of the [`ConditionType`]"));
    let lower_case_again = lower_case.clone();
    let lower_case_again_again = lower_case.clone();

    let mark = lower_case.clone().map(|l| Ident::new(&format!("mark_{l}"), l.span()));
    let mark_with_reason = lower_case.clone().map(|l| Ident::new(&format!("mark_{l}_with_reason"), l.span()));
    let mark_not = lower_case.clone().map(|l| Ident::new(&format!("mark_not_{l}"), l.span()));

    let condition_type_name = Ident::new(&format!("{name}Type"), name.span());
    let condition_type_doc = format!("A [`ConditionType`] that implement this trait duck types to [`{name}`].");
    let manager_name = Ident::new(&format!("{name}Manager"), name.span());
    let manager_doc = format!("Allows a status to manage [`{name}`].");

    quote! {
        #[doc = #condition_type_doc]
        pub trait #condition_type_name: ::knative_conditions::ConditionType {
            #(
                #[doc = #lower_case_doc]
                fn #lower_case() -> Self;
            )*
        }

        #[automatically_derived]
        impl #condition_type_name for #name {
            #(
                fn #lower_case_again() -> Self {
                    #name::#capitalized
                }
            )*
        }

        #[automatically_derived]
        impl ::knative_conditions::ConditionType for #name {
            fn happy() -> Self {
                #name::#happy
            }

            fn dependents() -> &'static [Self] {
                &[#(#name::#dependents_again),*]
            }
        }

        #[automatically_derived]
        impl Default for #name {
            fn default() -> Self {
                #name::#happy
            }
        }

        #[doc = #manager_doc]
        pub trait #manager_name<S>: ::knative_conditions::ConditionAccessor<S>
        where S: #condition_type_name {
            #(
                fn #mark(&mut self) {
                    self.manager().mark_true(S::#lower_case_again_again());
                }

                fn #mark_with_reason(&mut self, reason: &str, message: Option<String>) {
                    self.manager().mark_true_with_reason(S::#lower_case_again_again(), reason, message);
                }

                fn #mark_not(&mut self, reason: &str, message: Option<String>) {
                    self.manager().mark_false(S::#lower_case_again_again(), reason, message);
                }
            )*
        }

        impl<S: #condition_type_name, T: ::knative_conditions::ConditionAccessor<S> + ?Sized> #manager_name<S> for T {}
    }.into()
}

