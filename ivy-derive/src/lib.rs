use itertools::Itertools;
use proc_macro_crate::FoundCrate;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{
    Attribute, DeriveInput, Error, Field, Ident, Result, Type, Visibility, spanned::Spanned,
};

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
#[proc_macro_derive(Resource, attributes(resource))]
pub fn resource(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);

    resource_impl(input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

fn resource_impl(input: DeriveInput) -> Result<TokenStream> {
    let ident = input.ident.clone();
    let crate_name = proc_macro_crate::crate_name("ivy-assets")
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
    data_struct: &syn::DataStruct,
) -> Result<TokenStream> {
    let named_fields = match &data_struct.fields {
        syn::Fields::Named(fields) => fields,
        _ => {
            return Err(syn::Error::new(input.ident.span(), "Expected named fields"));
        }
    };

    let fields: Vec<ParsedField> = named_fields
        .named
        .iter()
        .map(ParsedField::get)
        .collect::<Result<_>>()?;

    let desc_fields = fields.iter().map(|f| {
        let ident = &f.ident;
        let vis = &f.vis;
        let ty = f.ty;
        if f.attrs.load {
            quote! { #vis #ident: <#ty as #crate_name::loadable::Resource>::Desc }
        } else {
            quote! { #vis #ident: #ty }
        }
    });

    let field_load = fields.iter().map(|f| {
        let ident = &f.ident;
        let ty = f.ty;

        if f.attrs.load {
            quote! {
                #ident: Loadable::load(&self.#ident, assets).await?
            }
        } else {
            quote! {
                #ident: self.#ident.clone()
            }
        }
    });

    let vis = &input.vis;
    let name = &input.ident;

    let desc_name = format_ident!("{}Desc", input.ident);

    let expanded = quote! {
        #[derive(serde::Serialize, serde::Deserialize)]
        #vis struct #desc_name {
            #(#desc_fields),*
        }
        impl #crate_name::loadable::Resource for #name {
            type Desc = #desc_name;
        }

        impl #crate_name::loadable::Loadable for #desc_name {
            type Output = #name;

            async fn load(&self, assets: &#crate_name::AssetCache) -> anyhow::Result<Self::Output> {
                Ok(#name {
                    #(#field_load),*
                })
            }
        }
    };

    Ok(expanded)
}

#[derive(Clone)]
struct ParsedField<'a> {
    vis: &'a Visibility,
    ty: &'a Type,
    ident: &'a Ident,
    attrs: FieldAttrs,
}

impl<'a> ParsedField<'a> {
    fn get(field: &'a Field) -> Result<Self> {
        let ident = field
            .ident
            .as_ref()
            .ok_or(Error::new(field.span(), "Only named fields are supported"))?;

        let attrs = FieldAttrs::get(&field.attrs)?;

        Ok(Self {
            vis: &field.vis,
            ty: &field.ty,
            ident,
            attrs,
        })
    }
}

#[derive(Default, Debug, Clone)]
struct FieldAttrs {
    load: bool,
}

impl FieldAttrs {
    fn get(input: &[Attribute]) -> Result<Self> {
        let mut res = Self::default();

        for attr in input {
            if !attr.path().is_ident("resource") {
                continue;
            }

            match &attr.meta {
                syn::Meta::List(list) => {
                    // Parse list

                    list.parse_nested_meta(|meta| {
                        // item = [Debug, PartialEq]
                        if meta.path.is_ident("load") {
                            res.load = true;
                            Ok(())
                        } else {
                            Err(Error::new(
                                meta.path.span(),
                                "Unknown fetch field attribute",
                            ))
                        }
                    })?;
                }
                _ => {
                    return Err(Error::new(
                        Span::call_site(),
                        "Expected a MetaList for `fetch`",
                    ));
                }
            };
        }

        Ok(res)
    }
}
