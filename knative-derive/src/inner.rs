use crate::{
    REQUIRED_VARIANTS,
    error::VerificationError
};
use proc_macro::TokenStream;
use quote::quote;
use syn::{
    spanned::Spanned,
    punctuated::Punctuated,
    token::Comma,
    Variant,
    Error,
    Result,
    Data::Enum,
    DeriveInput,
    Ident
};

fn is_dependent(variant: &syn::Variant) -> bool {
    variant.attrs
        .iter()
        .any(|a| a.path.segments.iter()
             .any(|p| p.ident == "dependent"))
}

fn verify_variants(variants: &Punctuated<Variant, Comma>) -> Result<()> {
    let mut one_required = false;

    for v in variants {
        let name = v.ident.to_string();
        if REQUIRED_VARIANTS.contains(&name.as_str()) {
            // Ensure top level conditions are not dependents
            if is_dependent(&v) {
                Err(VerificationError::NotDependent(name))?
            }
            // Ensure only one top level condition exists
            if !one_required {
                one_required = true;
            } else {
                Err(VerificationError::OneRequiredVariant)?
            }
        }
    }

    if !one_required {
        Err(VerificationError::OneRequiredVariant)?
    }

    Ok(())
}

pub fn inner_derive(ast: DeriveInput) -> Result<TokenStream> {
    let name = &ast.ident;

    let variants = match ast.data {
        Enum(syn::DataEnum { ref variants, .. }) => variants,
        _ => return Err(Error::new(
            ast.span(),
            "ConditionType may only be derived on enums"
        ))
    };

    verify_variants(variants)?;

    let happy = &variants.iter()
        .find(|v| REQUIRED_VARIANTS.contains(&v.ident.to_string().as_str()))
        .unwrap()
        .ident;
    let dependents = variants.iter()
        .filter(|v| is_dependent(v))
        .map(|v| &v.ident);

    let capitalized = variants.iter()
        .map(|v| v.ident.clone())
        .filter(|v| !REQUIRED_VARIANTS.contains(&v.to_string().as_str()));
    let lower_case = capitalized.clone()
        .map(|v| Ident::new(&v.to_string().to_lowercase(), v.span()));
    let lower_case_doc = capitalized.clone()
        .map(|c| format!("Returns the `{c}` variant of the [`ConditionType`]"));
    let lower_case_again = lower_case.clone();
    let lower_case_again_again = lower_case.clone();

    let mark = lower_case.clone().map(|l| Ident::new(&format!("mark_{l}"), l.span()));
    let mark_with_reason = lower_case.clone().map(|l| Ident::new(&format!("mark_{l}_with_reason"), l.span()));
    let mark_not = lower_case.clone().map(|l| Ident::new(&format!("mark_not_{l}"), l.span()));

    let condition_type_name = Ident::new(&format!("{name}Type"), name.span());
    let condition_type_doc = format!("A [`ConditionType`] that implement this trait duck types to [`{name}`].");
    let manager_name = Ident::new(&format!("{name}Manager"), name.span());
    let manager_doc = format!("Allows a status to manage [`{name}`].");

    Ok(quote! {
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
                #[inline]
                fn #lower_case_again() -> Self {
                    #name::#capitalized
                }
            )*
        }

        #[automatically_derived]
        impl ::knative_conditions::ConditionType for #name {
            #[inline]
            fn happy() -> Self {
                #name::#happy
            }

            #[inline]
            fn dependents() -> &'static [Self] {
                &[#(#name::#dependents),*]
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
    }.into())
}
