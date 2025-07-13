mod bundle;
mod editable;
mod resource;

use crate::{bundle::bundle_impl, resource::resource_impl};

/// Derives `Resource` and load for a type.
///
/// # Usage
///
/// ```rs
/// #[derive(Resource)]
/// struct MyResource {
///     #[resource(load)]
///     image: AssetPath<DynamicImage>,
///     size: IVec2,
/// }
/// ```
#[proc_macro_derive(Resource, attributes(resource, resource_attr))]
pub fn resource(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);

    resource_impl(input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

/// Registers a [`Resource`] as a component Bundle.
#[proc_macro_derive(Bundle)]
pub fn bundle(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);

    bundle_impl(input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

#[proc_macro_derive(Editable, attributes(editable))]
pub fn editable(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);

    editable::editable_impl(input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}
