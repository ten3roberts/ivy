use proc_macro_crate::FoundCrate;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{
    Attribute, DataEnum, DeriveInput, Error, Field, Ident, Index, Result, Token, Type, Visibility,
    bracketed, parse_quote_spanned, punctuated::Punctuated, spanned::Spanned,
};

pub fn resource_impl(input: DeriveInput) -> Result<TokenStream> {
    let ident = input.ident.clone();
    let crate_name = proc_macro_crate::crate_name("ivy-assets")
        .map_err(|_| syn::Error::new_spanned(ident, "Could not find crate `ivy-assets`"))?;

    let crate_name = match crate_name {
        FoundCrate::Itself => Ident::new("crate", Span::call_site()),
        FoundCrate::Name(name) => Ident::new(&name, Span::call_site()),
    };

    match &input.data {
        syn::Data::Struct(data_struct) => expand_struct(crate_name, &input, data_struct),
        syn::Data::Enum(data_enum) => expand_enum(crate_name, &input, data_enum),
        syn::Data::Union(_data_union) => todo!(),
    }
}

fn expand_enum(
    crate_name: Ident,
    input: &DeriveInput,
    data_enum: &DataEnum,
) -> Result<TokenStream> {
    let attrs = Attrs::get(&input.attrs)?;
    let ident = &input.ident;
    let enum_ident = ident;

    let desc_name = format_ident!("{}Desc", input.ident);

    let desc_variants = data_enum
        .variants
        .iter()
        .map(|variant| {
            let ident = &variant.ident;
            let attrs = Attrs::get(&variant.attrs)?;

            let desc_fields = match &variant.fields {
                syn::Fields::Named(fields) => {
                    let fields: Vec<ParsedField> = fields
                        .named
                        .iter()
                        .map(ParsedField::get)
                        .collect::<Result<_>>()?;

                    let desc_fields = expand_fields(&crate_name, ident, &fields, &attrs)?;
                    let load_fields = expand_load(&crate_name, &fields)?;
                    let field_names = fields.iter().map(|v| &v.ident);

                    (
                        quote! {
                            #ident { #(#desc_fields),* }
                        },
                        quote! {
                            Self::#ident { #(#field_names),* } => #enum_ident::#ident { #(#load_fields),* }
                        },
                    )
                }
                syn::Fields::Unnamed(fields) => {
                    let fields: Vec<IndexedField> = fields
                        .unnamed
                        .iter()
                        .enumerate()
                        .map(|(i, f)| IndexedField::get(i, f))
                        .collect::<Result<_>>()?;

                    let desc_fields = fields.iter().map(|f| {
                        let ty = &f.ty;
                        let named_ident = &f.named_ident;
                        let field_attrs = &f.attrs.attrs;

                        if f.attrs.load {
                            quote! {
                                #(#field_attrs)*
                                #named_ident: <#ty as #crate_name::loadable::Resource>::Desc
                            }
                        } else {
                            quote! {
                                #(#field_attrs)*
                                #named_ident: #ty
                            }
                        }
                    });

                    let load_fields = fields.iter().map(|f| {
                        let named_ident = &f.named_ident;

                        if f.attrs.load {
                            quote! {
                                #named_ident: Loadable::load(&self.#named_ident, assets).await?
                            }
                        } else {
                            quote! {
                                #named_ident: self.#named_ident.clone()
                            }
                        }
                    });

                    let named_ident = fields.iter().map(|f| &f.named_ident);

                    (
                        quote! { #ident(#(#desc_fields),*) },
                        quote! { Self::#ident(#(#named_ident),*) => #enum_ident::#ident(#(#load_fields),*) },
                    )
                }
                syn::Fields::Unit => (quote! { #ident }, quote! { Self::#ident => #enum_ident::#ident }),
            };

            Ok(desc_fields)
        })
        .collect::<Result<Vec<_>>>()?;

    let (desc_variants, desc_loads): (Vec<_>, Vec<_>) = desc_variants.into_iter().unzip();

    let extras = match &attrs.derives {
        Some(extras) => {
            quote! { #[derive(#extras)]}
        }
        None => quote! {},
    };

    let expanded = quote! {
        #[derive(Debug, Clone, #crate_name::registry::serde::Serialize, #crate_name::registry::serde::Deserialize)]
        #extras
        pub enum #desc_name {
            #(#desc_variants),*
        }

        impl #crate_name::loadable::Resource for #ident {
            type Desc = #desc_name;

            fn tag_name() -> &'static str {
                stringify!(#ident)
            }
        }

        impl #crate_name::loadable::Loadable for #desc_name {
            type Output = #ident;

            async fn load(&self, assets: &#crate_name::AssetCache) -> anyhow::Result<Self::Output> {
                Ok(match self {
                    #(#desc_loads),*
                })
            }
        }
    };

    Ok(expanded)
}

fn expand_fields(
    crate_name: &Ident,
    _: &Ident,
    fields: &[ParsedField],
    _attrs: &Attrs,
) -> Result<Vec<TokenStream>> {
    fields
        .iter()
        .map(|field| {
            let ty = &field.ty;
            let ident = &field.ident;
            let field_attrs = &field.attrs.attrs;

            if field.attrs.load {
                Ok(quote! {
                    #(#field_attrs)*
                    #ident: <#ty as #crate_name::loadable::Resource>::Desc
                })
            } else {
                Ok(quote! {
                    #(#field_attrs)*
                    #ident: #ty
                })
            }
        })
        .collect()
}

fn expand_load(crate_name: &Ident, fields: &[ParsedField]) -> Result<Vec<TokenStream>> {
    fields
        .iter()
        .map(|f| {
            let ident = &f.ident;

            if f.attrs.load {
                Ok(quote! {
                    #ident: #crate_name::loadable::Loadable::load(#ident, assets).await?
                })
            } else {
                Ok(quote! {
                    #ident: #ident.clone()
                })
            }
        })
        .collect()
}

fn expand_struct(
    crate_name: Ident,
    input: &DeriveInput,
    data_struct: &syn::DataStruct,
) -> Result<TokenStream> {
    let attrs = Attrs::get(&input.attrs)?;

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
        let field_attrs = &f.attrs.attrs;

        if f.attrs.load {
            quote! {
                #(#field_attrs)*
                #vis #ident: <#ty as #crate_name::loadable::Resource>::Desc
            }
        } else {
            quote! {
                #(#field_attrs)*
                #vis #ident: #ty
            }
        }
    });

    let field_load = fields.iter().map(|f| {
        let ident = &f.ident;

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
    let ident = &input.ident;

    let desc_name = format_ident!("{}Desc", input.ident);

    let name_str = ident.to_string();
    let extras = match &attrs.derives {
        Some(extras) => {
            quote! { #[derive(#extras)]}
        }
        None => quote! {},
    };
    let expanded = quote! {
        #[derive(Debug, Clone, #crate_name::registry::serde::Serialize, #crate_name::registry::serde::Deserialize)]
        #extras
        #vis struct #desc_name {
            #(#desc_fields),*
        }
        impl #crate_name::loadable::Resource for #ident {
            type Desc = #desc_name;

            fn tag_name() -> &'static str {
                #name_str
            }
        }

        #crate_name::registry::inventory::submit! {
            #crate_name::registry::ResourceRegistration::new::<#ident>(#name_str)
        }

        impl #crate_name::loadable::Loadable for #desc_name {
            type Output = #ident;

            async fn load(&self, assets: &#crate_name::AssetCache) -> anyhow::Result<Self::Output> {
                Ok(#ident {
                    #(#field_load),*
                })
            }
        }
    };

    Ok(expanded)
}

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

struct IndexedField<'a> {
    ty: &'a Type,
    index: Index,
    attrs: FieldAttrs,
    named_ident: Ident,
}

impl<'a> IndexedField<'a> {
    fn get(index: usize, field: &'a Field) -> Result<Self> {
        let attrs = FieldAttrs::get(&field.attrs)?;

        let named_ident = Ident::new(&format!("field_{index}"), Span::call_site());
        Ok(Self {
            ty: &field.ty,
            index: Index::from(index),
            named_ident,
            attrs,
        })
    }
}

#[derive(Default)]
struct FieldAttrs {
    load: bool,
    attrs: Vec<syn::Attribute>,
}

impl FieldAttrs {
    fn get(input: &[Attribute]) -> Result<Self> {
        let mut res = Self::default();

        for attr in input {
            if attr.path().is_ident("resource_attr") {
                match &attr.meta {
                    syn::Meta::List(meta_list) => {
                        // Parse into a syn::Attribute
                        let inner = &meta_list.tokens;
                        let original_span = attr.span();
                        let pound = attr.pound_token;
                        let inner_attr = parse_quote_spanned!(original_span=> #pound [ #inner ]);
                        res.attrs.push(inner_attr);
                    }
                    _ => {
                        return Err(Error::new(
                            Span::call_site(),
                            "Expected a MetaList for `resource_attr`",
                        ));
                    }
                }
                continue;
            }

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
                            Err(Error::new(attr.span(), "Unknown resource attribute"))
                        }
                    })?;
                }
                _ => {
                    return Err(Error::new(
                        Span::call_site(),
                        "Expected a MetaList for `resource`",
                    ));
                }
            };
        }

        Ok(res)
    }
}

#[derive(Default)]
struct Attrs {
    derives: Option<Punctuated<Ident, Token![,]>>,
}

impl Attrs {
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
                        if meta.path.is_ident("derive") {
                            let value = meta.value()?;
                            let content;
                            bracketed!(content in value);
                            let content =
                                <Punctuated<Ident, Token![,]>>::parse_terminated(&content)?;

                            res.derives = Some(content);
                            Ok(())
                        } else {
                            Err(Error::new(meta.path.span(), "Unknown resource attribute"))
                        }
                    })?;
                }
                _ => {
                    return Err(Error::new(
                        Span::call_site(),
                        "Expected a MetaList for `resource`",
                    ));
                }
            };
        }

        Ok(res)
    }
}
