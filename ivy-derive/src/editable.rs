use itertools::Itertools;
use proc_macro_crate::FoundCrate;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{Attribute, DeriveInput, Error, Field, Ident, Result, Type, spanned::Spanned};

pub fn editable_impl(input: DeriveInput) -> Result<TokenStream> {
    let ident = input.ident.clone();
    let crate_name = proc_macro_crate::crate_name("ivy-editable")
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
        .filter_ok(|v| !v.attrs.skip)
        .collect::<Result<_>>()?;

    let ident = &input.ident;

    let field_names = fields.iter().map(|f| &f.ident).collect_vec();
    let default = (0..field_names.len()).map(|_| quote! { None });

    let field_default = quote! {
        value.filter_map(
            |v| Some((#(Some(v.#field_names),)*)),
            |(#(#field_names,)*)| Some(#ident {
                #(#field_names: #field_names?),*
            })
        )
        .memo((#(#default,)*))
    };

    let field_lower = fields.iter().enumerate().map(|(i, f)| {
        let ident = &f.ident;

        let i = syn::Index::from(i);
        quote! {
            let #ident = state.clone().project_ref(|v| &v.#i, |v| &mut v.#i).lower_option();
        }
    });

    let field_project = fields.iter().map(|f| {
        let ident = &f.ident;

        quote! {
            let #ident = state.clone().project_ref(|v| &v.#ident, |v| &mut v.#ident);
        }
    });

    let field_edit = fields.iter().map(|f| {
        let ident = &f.ident;

        let ty = &f.ty;
        let label = quote! {
            #crate_name::__private::violet::core::widget::interactive::base::InteractiveWidget::new(
                #crate_name::__private::violet::core::widget::label(stringify!(#ident))
            ).with_tooltip_text(stringify!(#ty))
        };

        let editor = quote! {
            <#ty as #crate_name::Editable>::create_editor(#ident)
        };

        quote! {
            |scope: &mut #crate_name::__private::violet::core::Scope<'_>| {
                if <#ty as #crate_name::Editable>::INLINE {
                    #crate_name::__private::violet::core::widget::row((
                        #crate_name::__private::violet::core::widget::Stack::new(#label).with_maximize(#crate_name::__private::violet::glam::Vec2::X),
                        #editor
                    )).with_cross_align(#crate_name::__private::violet::core::layout::Align::Center).mount(scope);
                } else {
                    #crate_name::__private::violet::core::widget::Collapsible::new(
                        #label,
                        #editor
                    ).indent(true).mount(scope);
                }
            }
        }
    }).collect_vec();

    let expanded = quote! {
        impl #crate_name::Editable for #ident {
            const INLINE: bool = false;

            fn create_editor<S: 'static + Send + Sync + #crate_name::__private::violet::core::state::StateDuplex<Item = Self>>(
                value: S,
            ) -> Box<dyn Send + #crate_name::__private::violet::core::widget::Widget> {
                use #crate_name::__private::violet::core::widget::Widget;
                use #crate_name::__private::violet::core::style::SizeExt;
                use #crate_name::__private::violet::core::state::StateExt;

                let state = ::std::sync::Arc::new(#field_default);

                #(#field_lower)*

                Box::new(
                    #crate_name::__private::violet::core::widget::col( (#(#field_edit),*))
                )
            }

            fn create_editor_project<S: 'static + Send + Sync + Clone + #crate_name::__private::violet::core::state::StateStreamRef<Item = Self> + #crate_name::__private::violet::core::state::StateWrite<Item = Self>>(
                state: S,
            ) -> Box<dyn Send + #crate_name::__private::violet::core::widget::Widget> {
                use #crate_name::__private::violet::core::widget::Widget;
                use #crate_name::__private::violet::core::style::SizeExt;
                use #crate_name::__private::violet::core::state::StateExt;

                #(#field_project)*

                Box::new(
                    #crate_name::__private::violet::core::widget::col( (#(#field_edit),*))
                )
            }
        }

        #crate_name::register_editable!(#ident);

    };

    Ok(expanded)
}

#[derive(Clone)]
struct ParsedField<'a> {
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
            ty: &field.ty,
            ident,
            attrs,
        })
    }
}

#[derive(Default, Debug, Clone)]
struct FieldAttrs {
    skip: bool,
}

impl FieldAttrs {
    fn get(input: &[Attribute]) -> Result<Self> {
        let mut res = Self::default();

        for attr in input {
            if !attr.path().is_ident("editable") {
                continue;
            }

            match &attr.meta {
                syn::Meta::List(list) => {
                    // Parse list

                    list.parse_nested_meta(|meta| {
                        // item = [Debug, PartialEq]
                        if meta.path.is_ident("skip") {
                            res.skip = true;
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
