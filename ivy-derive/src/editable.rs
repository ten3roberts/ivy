use itertools::Itertools;
use proc_macro_crate::FoundCrate;
use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, format_ident, quote};
use syn::{
    Attribute, DeriveInput, Error, Expr, Field, Ident, Index, PatLit, Result, Token, Type,
    parenthesized, spanned::Spanned,
};

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

    let mut inline = true;

    let disc_pat = data_enum.variants.iter().map(|v| {
        let ident = &v.ident;
        let ident_s = ident.to_string();

        let pat = match &v.fields {
            syn::Fields::Named(named) => {
                inline = false;
                let names = named.named.iter().map(|v| {
                    let ident = &v.ident;
                    quote! {#ident: _}
                });

                quote! { { #(#names),* } }
            }
            syn::Fields::Unnamed(fields_unnamed) => {
                inline = false;
                let repeat = (0..fields_unnamed.unnamed.len()).map(|_| quote! { _ });
                quote! { (#(#repeat),*) }
            }
            syn::Fields::Unit => quote! {},
        };
        quote! { Self::#ident #pat => #ident_s }
    });

    let violet = quote! { #crate_name::__private::violet::core };

    let default_variant = data_enum
        .variants
        .first()
        .map(|v| &v.ident)
        .ok_or_else(|| syn::Error::new_spanned(ident, "No variants found in enum"))?
        .to_string();

    let discriminant = quote! {
        ::std::sync::Arc::new(state.clone().filter_map(|v| Some( match v { #(#disc_pat),* }), |_| None).memo(#default_variant).dedup()) as ::std::sync::Arc<dyn Send + Sync + #violet::state::StateDuplex<Item = &'static str>>;
    };

    let variant_names = data_enum.variants.iter().map(|v| {
        let ident = &v.ident;
        let ident_s = ident.to_string();

        quote! { #violet::widget::DisplayWidget::new(#ident_s) }
    });

    let kind_selection = quote! { #violet::widget::interactive::Dropdown::new(discriminant.clone().map_value(#violet::widget::DisplayWidget::new, |v| v.value().clone()), [#(#variant_names),*]); };

    let variant_editors = |project| -> syn::Result<Vec<TokenStream>> {
        data_enum
        .variants
        .iter()
        .map(|v| {
            let ident = &v.ident;
            let ident_s = ident.to_string();

            let body = match &v.fields {
                syn::Fields::Named(fields_named) => {
                    let fields: Vec<ParsedField> = fields_named
                        .named
                        .iter()
                        .map(ParsedField::get)
                        .try_collect()?;

                    let field_names = fields.iter().map(|v| v.ident).collect_vec();

                    let field_lower = fields.iter().enumerate().map(|(i, f)| {
                        let ident = &f.ident;

                        let ty = &f.ty;
                        let i = syn::Index::from(i);
                        quote! {
                            let #ident = Box::new(state.clone().project_ref(|v| &v.#i, |v| &mut v.#i).lower_option()) as Box<dyn Send+Sync+#violet::state::StateDuplex<Item = #ty>>;
                        }
                    }).collect_vec();

                    let field_editors = expand_field_editors(&crate_name, &fields, project);

                    let destruct =quote! { { #(#field_names,)* } };
                    let destruct_tuple =quote! { ( #(#field_names,)* ) };
                    let to_tuple = quote! {  (#(Some(#field_names),)*) };
                    let from_tuple = quote! { { #(#field_names: #field_names?),* } };

                    let matched_state = if project {
                        quote! {
                            // Try and destructure this variant
                            let state = ::std::sync::Arc::new(#violet::StateExt::filter_map(
                                    state.clone(),
                                    |v| if let Self::#ident #destruct = v { Some(#to_tuple) } else { None },
                                    |#destruct_tuple| Some(Self::#ident #from_tuple)
                                )
                            );
                        }

                    } else {
                        let default = fields.iter().map(|f| {
                            if let Some(default_expr) = &f.attrs.default {
                                quote! { Some(#default_expr) }
                            } else {
                                quote! { None }
                            }
                        }).collect_vec();

                        quote!{
                            // Try and destructure this variant
                            let state = ::std::sync::Arc::new(#violet::StateExt::memo(#violet::StateExt::filter_map(state.clone(),
                                    |v| if let Self::#ident #destruct = v { Some(#to_tuple) } else { None },
                                    |#destruct_tuple| Some(Self::#ident #from_tuple)),
                                (#(#default,)*)));
                        }
                    };

                    quote! {
                        #matched_state

                        state.sync_initial();
                        #(#field_lower)*

                        Box::new(
                            #violet::widget::col( (#(#field_editors),*))
                        ) as Box<dyn Send + Widget>
                    }
                }
                syn::Fields::Unnamed(fields) => {
                    let fields: Vec<IndexedField> = fields.unnamed
                        .iter().enumerate()
                        .map(|(i, v)| IndexedField::get(i,v))
                        .try_collect()?;

                    let field_names = fields.iter().map(|v| &v.named_ident).collect_vec();


                    let destruct =quote! { ( #(#field_names,)* ) };
                    let destruct_tuple =quote! { ( #(#field_names,)* ) };
                    let to_tuple = quote! {  (#(Some(#field_names),)*) };
                    let from_tuple = quote! { ( #(#field_names?),* ) };
                    let matched_state = if project {
                        quote! {
                            let state = ::std::sync::Arc::new(state.clone()
                                // Try and destructure this variant
                                .filter_map(
                                    |v| if let Self::#ident #destruct = v { Some(#to_tuple) } else { None },
                                    |#destruct_tuple| Some(Self::#ident #from_tuple)));
                        }
                    } else {

                        let default = fields.iter().map(|f| {
                            if let Some(default_expr) = &f.attrs.default {
                                quote! { Some(#default_expr) }
                            } else {
                                quote! { None }
                            }
                        }).collect_vec();


                        quote! {

                            let state = ::std::sync::Arc::new(state.clone()
                                // Try and destructure this variant
                                .filter_map(
                                    |v| if let Self::#ident #destruct = v { Some(#to_tuple) } else { None },
                                    |#destruct_tuple| Some(Self::#ident #from_tuple))
                                .memo((#(#default,)*)));
                            }
                    };

                    let field_lower = fields.iter().map(|f| {
                        let index = &f.index;
                        let ty = &f.ty;

                        let named_ident = &f.named_ident;
                        quote! {
                            let #named_ident = Box::new(state.clone().project_ref(|v| &v.#index, |v| &mut v.#index).lower_option()) as Box<dyn Send+Sync+#violet::state::StateDuplex<Item = #ty>>;
                        }
                    }).collect_vec();

                    let field_editors = expand_field_editors_indexed(&crate_name, &fields, false);

                    quote! [
                        #matched_state
                        state.sync_initial();
                    #(#field_lower)*

                    Box::new(
                        #violet::widget::col( (#(#field_editors),*))
                    ) as Box<dyn Send + Widget>
                    ]
                },
                syn::Fields::Unit => {

                    quote! {
                        let state = ::std::sync::Arc::new(state.clone()
                            // Try and destructure this variant
                            .filter_map(
                                |v| if let Self::#ident = v { Some(()) } else { None },
                                |()| Some(Self::#ident))
                            .memo(()));

                        state.sync_initial();
                        Box::new(#violet::widget::EmptyWidget) as Box<dyn Send + Widget> }
                },
            };

            syn::Result::Ok(quote! { #ident_s => { #body } })
        })
        .try_collect()
    };

    let variant_editors = variant_editors(false)?;

    let value_editor = quote! {
        #crate_name::__private::futures::StreamExt::map(discriminant.stream(), {let assets = assets.clone(); move |disc| {
            match disc {
                #(#variant_editors,)*
                _ => unreachable!()
            }
        }})
    };

    let expanded = quote! {
        impl #crate_name::Editable for #ident {
            const INLINE: bool = #inline;

            fn create_editor<S: 'static + Send + Sync + #violet::state::StateDuplex<Item = Self>>(
                state: S,
                assets: &#crate_name::AssetCache,
            ) -> Box<dyn Send + #violet::widget::Widget> {
                use #violet::{StateExt, StateStream, Widget, style::SizeExt};
                let state = ::std::sync::Arc::new(state);


                let discriminant = #discriminant;
                let kind_selection = #kind_selection;
                let value_editor = #value_editor;
                Box::new(#violet::widget::col((kind_selection, #violet::widget::StreamWidget::new(value_editor))))
            }

            fn create_editor_project<S: 'static + Send + Sync + Clone + #violet::state::StateStreamRef<Item = Self> + #violet::state::StateWrite<Item = Self>>(
                state: S,
                assets: &#crate_name::AssetCache,
            ) -> Box<dyn Send + #violet::widget::Widget> {
                <Self as #crate_name::Editable>::create_editor(#violet::state::StateExt::project_ref(state, |v| v, |v| v), assets)
            }
        }

        #crate_name::register_editable!(#ident);

    };

    Ok(expanded)
}

fn expand_field_editors(
    crate_name: &Ident,
    fields: &[ParsedField],
    project: bool,
) -> Vec<TokenStream> {
    let violet = quote! { #crate_name::__private::violet::core };
    fields.iter().map(|f| {
        let ident = &f.ident;

        let ty = &f.ty;
        let tooltip = if !f.doc.is_empty() {
            let doc =  &f.doc;
            quote! {#doc}
        } else {
            let ty = &f.ty;
            quote! {stringify!(#ty)}
        };

        use heck::ToTitleCase;
        let name = ident.to_string().to_title_case();
        let label = quote! {
            #violet::widget::interactive::base::InteractiveWidget::new(
                #violet::widget::label(#name)
            ).with_tooltip_text(#tooltip)
        };

        let editor = if let Some(opts) = f.attrs.opts_tokens(crate_name) {
            let method = if project {
                format_ident!("create_editor_project_opts")
            } else {
                format_ident!("create_editor_opts")
            };
            quote! {
                <#ty as #crate_name::EditableWithOpts>::#method(#ident, #opts, assets)
            }
        } else {
            let method = if project {
                format_ident!("create_editor_project")
            } else {
                format_ident!("create_editor")
            };
            quote! {
                <#ty as #crate_name::Editable>::#method(#ident, assets)
            }
        };

        quote! {
            {
                let assets = assets.clone();
                move |scope: &mut #violet::Scope<'_>| {
                    let assets = &assets;
                    if <#ty as #crate_name::Editable>::INLINE {
                        #violet::widget::row((
                                #violet::widget::Stack::new(#label).with_maximize(#crate_name::__private::violet::glam::Vec2::X),
                                #editor
                        )).with_cross_align(#violet::layout::Align::Center).mount(scope);
                    } else {
                        #violet::widget::Collapsible::new(
                            #label,
                            #editor
                        ).indent(true).mount(scope);
                    }
                }
            }
        }
    }).collect_vec()
}

fn expand_field_editors_indexed(
    crate_name: &Ident,
    fields: &[IndexedField],
    project: bool,
) -> Vec<TokenStream> {
    let violet = quote! { #crate_name::__private::violet::core };

    fields
        .iter()
        .map(|f| {
            let ty = &f.ty;

            let named_ident = &f.named_ident;

            let editor = if let Some(opts) = f.attrs.opts_tokens(crate_name) {
                let method = if project {
                    format_ident!("create_editor_project_opts")
                } else {
                    format_ident!("create_editor_opts")
                };
                quote! {
                    <#ty as #crate_name::EditableWithOpts>::#method(#named_ident, #opts, assets)
                }
            } else {
                let method = if project {
                    format_ident!("create_editor_project")
                } else {
                    format_ident!("create_editor")
                };
                quote! {
                    <#ty as #crate_name::Editable>::#method(#named_ident, assets)
                }
            };

            quote! {
                {
                    let assets = assets.clone();
                    move |scope: &mut #violet::Scope<'_>| {
                        let assets = &assets;
                        if <#ty as #crate_name::Editable>::INLINE {
                            #violet::widget::row((
                                    #editor
                            )).with_cross_align(#violet::layout::Align::Center).mount(scope);
                        } else {
                            #violet::widget::Collapsible::new(
                                #violet::widget::label(""),
                                #editor
                            ).indent(true).mount(scope);
                        }
                    }
                }
            }
        })
        .collect_vec()
}

fn expand_struct(
    crate_name: Ident,
    input: &DeriveInput,
    data_struct: &syn::DataStruct,
) -> Result<TokenStream> {
    let violet = quote! { #crate_name::__private::violet::core };
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
        let ty = &f.ty;

        let i = syn::Index::from(i);
        quote! {
            let #ident = Box::new(state.clone().project_ref(|v| &v.#i, |v| &mut v.#i).lower_option()) as Box<dyn Send+Sync+#violet::state::StateDuplex<Item = #ty>>;
        }
    });

    let field_project = fields.iter().map(|f| {
        let ident = &f.ident;

        quote! {
            let #ident = state.clone().project_ref(|v| &v.#ident, |v| &mut v.#ident);
        }
    });

    let field_edit = expand_field_editors(&crate_name, &fields, false);
    let field_edit_project = expand_field_editors(&crate_name, &fields, true);

    let expanded = quote! {
        impl #crate_name::Editable for #ident {
            const INLINE: bool = false;

            fn create_editor<S: 'static + Send + Sync + #violet::state::StateDuplex<Item = Self>>(
                value: S,
                assets: &#crate_name::AssetCache,
            ) -> Box<dyn Send + #violet::widget::Widget> {
                use #violet::{StateExt, StateStream, Widget, style::SizeExt};
                let state = ::std::sync::Arc::new(#field_default);

                state.sync_initial();

                #(#field_lower)*

                let assets = assets.clone();
                Box::new(
                    #violet::widget::col( (#(#field_edit),*))
                )
            }

            fn create_editor_project<S: 'static + Send + Sync + Clone + #violet::state::StateStreamRef<Item = Self> + #violet::state::StateWrite<Item = Self>>(
                state: S,
                assets: &#crate_name::AssetCache,
            ) -> Box<dyn Send + #violet::widget::Widget> {
                use #violet::{StateExt, StateStream, Widget, style::SizeExt};
                let assets = assets.clone();
                #(#field_project)*

                Box::new(
                    #violet::widget::col( (#(#field_edit_project),*))
                )
            }
        }

        #crate_name::register_editable!(#ident);

    };

    Ok(expanded)
}

struct ParsedField<'a> {
    ty: &'a Type,
    ident: &'a Ident,
    attrs: FieldAttrs,
    doc: String,
}

impl<'a> ParsedField<'a> {
    fn get(field: &'a Field) -> Result<Self> {
        let ident = field
            .ident
            .as_ref()
            .ok_or(Error::new(field.span(), "Only named fields are supported"))?;

        let attrs = FieldAttrs::get(&field.attrs)?;

        Ok(Self {
            doc: attrs_to_doc(&field.attrs),
            ty: &field.ty,
            ident,
            attrs,
        })
    }
}

fn attrs_to_doc(attrs: &[Attribute]) -> String {
    attrs
        .iter()
        .filter_map(|v| match &v.meta {
            syn::Meta::NameValue(name_value) if name_value.path.is_ident("doc") => {
                if let Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(lit_str),
                    ..
                }) = &name_value.value
                {
                    Some(lit_str.value().trim().to_string())
                } else {
                    None
                }
            }
            _ => None,
        })
        .join("\n")
}

struct IndexedField<'a> {
    ty: &'a Type,
    index: Index,
    attrs: FieldAttrs,
    named_ident: Ident,
    doc: String,
}

impl<'a> IndexedField<'a> {
    fn get(index: usize, field: &'a Field) -> Result<Self> {
        let attrs = FieldAttrs::get(&field.attrs)?;

        let named_ident = Ident::new(&format!("field_{index}"), Span::call_site());

        Ok(Self {
            doc: attrs_to_doc(&field.attrs),
            ty: &field.ty,
            index: Index::from(index),
            named_ident,
            attrs,
        })
    }
}

#[derive(Default, Debug, Clone)]
struct FieldAttrs {
    skip: bool,
    range: Option<(Expr, Expr)>,
    default: Option<Expr>,
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
                        } else if meta.path.is_ident("range") {
                            let content;

                            parenthesized!(content in meta.input);
                            let start = content.parse()?;
                            content.parse::<Token![,]>()?;
                            let end = content.parse()?;
                            res.range = Some((start, end));
                            Ok(())
                        } else {
                            Err(Error::new(
                                meta.path.span(),
                                "Unknown editable field attribute",
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

    fn opts_tokens(&self, crate_name: &Ident) -> Option<TokenStream> {
        if let Some((start, end)) = &self.range {
            Some(quote! {
                #crate_name::EditorOpts {
                    range: Some((#start, #end)),
                }
            })
        } else {
            None
        }
    }
}
