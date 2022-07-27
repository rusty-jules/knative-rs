//! Derive [`ConditionType`] on your own types to adhere to the Knative Source schema and condition
//! management.
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

    let variants_with_attrs = variants.clone().into_iter()
        .filter(|v| !v.attrs.is_empty())
        .map(|v| v.ident);
    let num_attrs: usize = variants_with_attrs.clone().count();

    let capitalized = variants.iter().map(|v| v.ident.clone());

    let search = capitalized.clone().map(|v| format!("{}", v)).collect::<Vec<String>>();
    let s = search.iter().map(|v| v.as_str()).collect::<Vec<&str>>();

    let (non_existent, non_existent_lower_case, existent, existent_lower_case) = if s.contains(&"Ready") && s.contains(&"Succeeded") {
         panic!("ConditionType must contain either Ready or Succeeded variant")
    } else if s.contains(&"Ready") {
        (Ident::new("Succeeded", name.span()), Ident::new("succeeded", name.span()), Ident::new("Ready", name.span()), Ident::new("ready", name.span()))
    } else if s.contains(&"Succeeded") {
        (Ident::new("Ready", name.span()), Ident::new("ready", name.span()), Ident::new("Succeeded", name.span()), Ident::new("succeeded", name.span()))
    } else {
        panic!("ConditionType must contain either Ready or Succeeded variant")
    };

    let capitalized = capitalized.filter(|v| v.to_string() != "Ready" && v.to_string() != "Succeeded");
    let lower_case = capitalized.clone()
        .map(|v| Ident::new(&format!("{}", v).to_lowercase(), v.span()));

    quote! {
        impl #name {
            #(
                pub fn #lower_case() -> Self {
                    #name::#capitalized
                }
            )*
        }

        impl ::knative_conditions::ConditionType<#num_attrs> for #name {
            //type Dependent = Self;

            fn happy() -> Self {
                #name::#existent
            }

            fn dependents() -> [Self; #num_attrs] {
                [#(#name::#variants_with_attrs),*]
            }

            fn #existent_lower_case() -> Self {
                #name::#existent
            }

            fn #non_existent_lower_case() -> Self {
                panic!(stringify!(#name does not contain #non_existent))
            }
        }

        impl Default for #name {
            fn default() -> Self {
                #name::#existent
            }
        }

        impl ::std::fmt::Display for #name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                ::std::write!(f, "{:?}", self)
            }
        }
    }.into()
}

