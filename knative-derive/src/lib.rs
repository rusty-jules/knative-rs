//! Derive [`ConditionType`] on your own types to adhere to the Knative Source schema and condition
//! management.
use proc_macro2::TokenStream;
use proc_macro2_diagnostics::{SpanDiagnosticExt, Diagnostic};
use quote::quote;
use syn::{
    spanned::Spanned,
    Data::Enum,
    DeriveInput,
    Ident
};

#[proc_macro_derive(ConditionType, attributes(dependent))]
pub fn wrapper(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    match derive(input) {
        Ok(tokens) => tokens.into(),
        Err(diag) => diag.emit_as_expr_tokens().into()
    }
}

fn derive(input: proc_macro::TokenStream) -> Result<TokenStream, Diagnostic> {
    let ast = syn::parse_macro_input::parse::<DeriveInput>(input.clone())
        .map_err(|e| {
            let tokens: TokenStream = input.into();
            tokens.span().error(format!("{}", e))
        })?;

    let name = &ast.ident;
    let variants = match ast.data {
        Enum(syn::DataEnum { ref variants, .. }) => variants,
        _ => panic!("ConditionType may only be derived on enums")
    };

    let first_var = if variants.is_empty() {
        return Err(name.span().error("Requires at least one variant"))
    } else {
        variants.iter().nth(0).unwrap().clone()
    };

    let variants_with_attrs = variants.clone().into_iter()
        .filter(|v| !v.attrs.is_empty())
        .map(|v| v.ident);
    let num_attrs: usize = variants_with_attrs.clone().count();

    let capitalized = variants.iter().map(|v| v.ident.clone());

    let search = capitalized.clone().map(|v| format!("{}", v)).collect::<Vec<String>>();
    let s = search.iter().map(|v| v.as_str()).collect::<Vec<&str>>();

    let (non_existent, non_existent_lower_case, existent, existent_lower_case) = if s.contains(&"Ready") && s.contains(&"Succeeded") {
         Err(first_var.span().error("ConditionType must contain either Ready or Succeeded variant"))?
    } else if s.contains(&"Ready") {
        (Ident::new("Succeeded", name.span()), Ident::new("succeeded", name.span()), Ident::new("Ready", name.span()), Ident::new("ready", name.span()))
    } else {
        (Ident::new("Ready", name.span()), Ident::new("ready", name.span()), Ident::new("Succeeded", name.span()), Ident::new("succeeded", name.span()))
    };

    let capitalized = capitalized.filter(|v| v.to_string() != "Ready" && v.to_string() != "Succeeded");
    let lower_case = capitalized.clone()
        .map(|v| Ident::new(&format!("{}", v).to_lowercase(), v.span()));

    Ok(quote! {
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
    })
}

