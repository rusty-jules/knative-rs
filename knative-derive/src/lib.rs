//! Derive [`ConditionType`] on your own types to adhere to the Knative Source schema and condition
//! management. Derives [`Default`] on your type, which must be the happy condition.
//!
//! [`ConditionType`]: ../knative_conditions/trait.ConditionType.html
use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input,
    Data::Enum,
    DeriveInput,
    Ident
};

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
    let lower_case_again = lower_case.clone();
    let lower_case_again_again = lower_case.clone();

    let mark = lower_case.clone().map(|l| Ident::new(&format!("mark_{l}"), l.span()));
    let mark_not = lower_case.clone().map(|l| Ident::new(&format!("mark_not_{l}"), l.span()));

    let condition_type_name = Ident::new(&format!("{name}Type"), name.span());
    let manager_name = Ident::new(&format!("{name}Manager"), name.span());

    quote! {
            pub trait #condition_type_name: ::knative_conditions::ConditionType {
                #(
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

        impl Default for #name {
            fn default() -> Self {
                #name::#happy
            }
        }

        /// Allows a status to manage [`#manager_name`]
        pub trait #manager_name<S>: ::knative_conditions::ConditionAccessor<S>
        where S: #condition_type_name {
            #(
                fn #mark(&mut self) {
                    self.manager().mark_true(S::#lower_case_again_again());
                }

                fn #mark_not(&mut self, reason: &str, message: Option<String>) {
                    self.manager().mark_false(S::#lower_case_again_again(), reason, message);
                }
            )*
        }

        impl<S: #condition_type_name, T: ::knative_conditions::ConditionAccessor<S> + ?Sized> #manager_name<S> for T {}
    }.into()
}

