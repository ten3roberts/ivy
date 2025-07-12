use proc_macro_crate::FoundCrate;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{DeriveInput, Ident, Result};

pub fn bundle_impl(input: DeriveInput) -> Result<TokenStream> {
    let ident = input.ident.clone();
    let crate_name = proc_macro_crate::crate_name("ivy-core")
        .map_err(|_| syn::Error::new_spanned(ident, "Could not find crate `ivy-assets`"))?;

    let crate_name = match crate_name {
        FoundCrate::Itself => Ident::new("crate", Span::call_site()),
        FoundCrate::Name(name) => Ident::new(&name, Span::call_site()),
    };

    match &input.data {
        syn::Data::Struct(data_struct) => expand_struct(crate_name, &input, data_struct),
        syn::Data::Enum(_data_enum) => todo!(),
        syn::Data::Union(_data_union) => todo!(),
    }
}

fn expand_struct(
    crate_name: Ident,
    input: &DeriveInput,
    _data_struct: &syn::DataStruct,
) -> Result<TokenStream> {
    let ident = &input.ident;

    let desc_name = format_ident!("{}Desc", input.ident);

    let name_str = ident.to_string();
    let expanded = quote! {
        impl #crate_name::template::BundleDesc for #desc_name {}

        #crate_name::bundle_registry::__private::inventory::submit! {
            #crate_name::bundle_registry::BundleRegistration::new::<#ident>(#name_str)
        }
    };

    Ok(expanded)
}
