use itertools::Itertools;
use proc_macro_crate::FoundCrate;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{Attribute, DeriveInput, Error, Field, Ident, Result, Token, Type, spanned::Spanned};

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
        syn::Data::Enum(data_enum) => expand_enum(crate_name, &input, data_enum),
        syn::Data::Union(_data_union) => todo!(),
    }
}

fn expand_enum(
    crate_name: Ident,
    input: &DeriveInput,
    data_enum: &syn::DataEnum,
) -> Result<TokenStream> {
    let ident = &input.ident;

    let disc_selection = data_enum.variants.iter().map(|v| {
        let ident = &v.ident;
        let ident_s = ident.to_string();

        let pat = match &v.fields {
            syn::Fields::Named(named) => quote! { {..} },
            syn::Fields::Unnamed(fields_unnamed) => {
                let repeat = (0..fields_unnamed.unnamed.len()).map(|_| quote! { _ });
                quote! { (#(#repeat),*) }
            }
            syn::Fields::Unit => quote! {},
        };
        quote! { Self::#ident #pat => #ident_s }
    });

    let disc_selection = quote! {
        Arc::new(state.clone().filter_map(|v| Some(Some( match v { #(#disc_selection),* })), |_| None).memo(None).dedup().lower_option());
    };

    let kind_selection = data_enum.variants.iter().map(|v| {
        let ident = &v.ident;
        let ident_s = ident.to_string();

        quote! { Selectable::new_value(label(#ident_s), discriminant.clone(), #ident_s) }
    });

    let kind_selection = quote! { row((#(#kind_selection),*)); };

    let variant_editors: Vec<_> = data_enum
        .variants
        .iter()
        .map(|v| {
            let ident = &v.ident;
            let ident_s = ident.to_string();

            let (destruct, assemble, field_editors) = match &v.fields {
                syn::Fields::Named(fields_named) => {
                    let fields: Vec<ParsedField> = fields_named
                        .named
                        .iter()
                        .map(ParsedField::get)
                        .try_collect()?;

                    let field_names = fields.iter().map(|v| v.ident).collect_vec();

                    // TODO: lower field states
                    let field_editors = expand_field_editors(&crate_name, &fields);

                    (
                        quote! { { #(#field_names),* } },
                        quote! {  (#(#field_names,)*) },
                        field_editors,
                    )
                }
                syn::Fields::Unnamed(_fields_unnamed) => todo!(),
                syn::Fields::Unit => unreachable!(),
            };

            let body = quote! {
                let state = Arc::new(state.clone()
                        .filter_map(
                            |v| if let Self::#ident #destruct = v { Some(#assemble) } else { None },
                            |#assemble| Some(Self::#ident #destruct))
                    )
                    .memo(Default::default());

                Box::new(
                    #crate_name::__private::violet::core::widget::col( (#(#field_editors),*))
                ) as Box<dyn Send+Widget>
            };

            syn::Result::Ok(quote! { #ident_s => { #body } })
        })
        .try_collect()?;

    let value_editor = quote! {
        discriminant.stream().map(move |disc| {
            match disc {
                #(#variant_editors,)*
                _ => unreachable!()
            }
        })
    };

    let expanded = quote! {
        impl #crate_name::Editable for #ident {
            const INLINE: bool = false;

            fn create_editor<S: 'static + Send + Sync + #crate_name::__private::violet::core::state::StateDuplex<Item = Self>>(
                state: S,
            ) -> Box<dyn Send + #crate_name::__private::violet::core::widget::Widget> {
                use #crate_name::__private::violet::core::widget::{ Widget, label, col, row, Selectable, StreamWidget };
                use #crate_name::__private::violet::core::style::SizeExt;
                use #crate_name::__private::violet::core::state::StateExt;
                use ::std::sync::Arc;

                let state = ::std::sync::Arc::new(state);


                let discriminant = #disc_selection;
                let kind_selection = #kind_selection;
                let value_editor = #value_editor;
                Box::new(col((kind_selection, StreamWidget::new(value_editor))))
            }

            fn create_editor_project<S: 'static + Send + Sync + Clone + #crate_name::__private::violet::core::state::StateStreamRef<Item = Self> + #crate_name::__private::violet::core::state::StateWrite<Item = Self>>(
                state: S,
            ) -> Box<dyn Send + #crate_name::__private::violet::core::widget::Widget> {
                use #crate_name::__private::violet::core::widget::Widget;
                use #crate_name::__private::violet::core::style::SizeExt;
                use #crate_name::__private::violet::core::state::StateExt;
                use #crate_name::__private::violet::core::state::StateStream;

                todo!()
            }
        }

        #crate_name::register_editable!(#ident);

    };
    // let expanded = quote! {

    //     let kind_selection = #kind_selection;
    //     let value_editor = #value_editor;
    //     Box::new(col((kind_selection, value_editor)))
    // };

    Ok(expanded)
}

fn expand_field_editors(crate_name: &Ident, fields: &[ParsedField]) -> Vec<TokenStream> {
    fields.iter().map(|f| {
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
    }).collect_vec()
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
    let default = fields.iter().map(|f| {
        if let Some(default_expr) = &f.attrs.default {
            quote! { Some(#default_expr) }
        } else {
            quote! { None }
        }
    });

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

    let field_edit = expand_field_editors(&crate_name, &fields);

    let field_edit_project = fields.iter().map(|f| {
        let ident = &f.ident;

        let ty = &f.ty;
        let label = quote! {
            #crate_name::__private::violet::core::widget::interactive::base::InteractiveWidget::new(
                #crate_name::__private::violet::core::widget::label(stringify!(#ident))
            ).with_tooltip_text(stringify!(#ty))
        };

        let editor = quote! {
            <#ty as #crate_name::Editable>::create_editor_project(#ident)
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

                state.sync_initial();

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
                    #crate_name::__private::violet::core::widget::col( (#(#field_edit_project),*))
                )
            }
        }

        #crate_name::register_editable!(#ident);

    };

    eprintln!("Expanded editable for {}: {}", ident, expanded);

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
    default: Option<syn::Expr>,
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
                        } else if meta.path.is_ident("default") {
                            if meta.input.peek(Token![=]) {
                                meta.value()?;
                                let expr = meta.input.parse()?;
                                res.default = Some(expr);
                            } else {
                                res.default = Some(syn::parse_quote! { Default::default() });
                            }

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

        eprintln!("Parsed field attributes: {:?}", res);
        Ok(res)
    }
}
