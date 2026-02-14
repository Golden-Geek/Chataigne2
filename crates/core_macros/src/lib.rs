use std::collections::BTreeMap;

use proc_macro::TokenStream;
use proc_macro2::{Delimiter, TokenTree};
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{
    parse_macro_input, parse_quote, Error, Expr, Field, Fields, GenericArgument, Ident, ImplItem,
    Item, ItemImpl, ItemStruct, LitInt, LitStr, PathArguments, Result, Token, Type,
};

#[derive(Clone)]
struct DelegatePath {
    segments: Vec<Ident>,
}

impl Parse for DelegatePath {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut segments = Vec::new();
        segments.push(input.parse::<Ident>()?);

        while input.peek(Token![.]) {
            input.parse::<Token![.]>()?;
            segments.push(input.parse::<Ident>()?);
        }

        Ok(Self { segments })
    }
}

struct NodeAttr {
    type_name: Option<LitStr>,
    via: Option<DelegatePath>,
}

impl Parse for NodeAttr {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut type_name = None;
        let mut via = None;

        while !input.is_empty() {
            if input.peek(LitStr) {
                if type_name.is_some() {
                    return Err(Error::new(input.span(), "duplicate node type literal"));
                }
                type_name = Some(input.parse::<LitStr>()?);
            } else if input.peek(Ident) {
                let key = input.parse::<Ident>()?;
                if key == "via" {
                    if via.is_some() {
                        return Err(Error::new(key.span(), "duplicate `via` argument"));
                    }
                    input.parse::<Token![=]>()?;
                    via = Some(input.parse::<DelegatePath>()?);
                } else {
                    return Err(Error::new(
                        key.span(),
                        "unsupported argument, expected string literal or `via = field.path`",
                    ));
                }
            } else {
                return Err(Error::new(
                    input.span(),
                    "unexpected attribute arguments, expected string literal or `via = field.path`",
                ));
            }

            if input.is_empty() {
                break;
            }

            input.parse::<Token![,]>()?;
            if input.is_empty() {
                break;
            }
        }

        Ok(Self { type_name, via })
    }
}

struct UpdateAttr {
    rate_hz: LitInt,
}

impl Parse for UpdateAttr {
    fn parse(input: ParseStream) -> Result<Self> {
        let rate_hz = input.parse::<LitInt>()?;
        if !input.is_empty() {
            return Err(Error::new(
                input.span(),
                "unexpected tokens, expected a single integer like #[update(60)]",
            ));
        }
        Ok(Self { rate_hz })
    }
}

#[derive(Default)]
struct ParamFieldArgs {
    default: Option<Expr>,
    decl_id: Option<LitStr>,
    label: Option<LitStr>,
    description: Option<LitStr>,
}

impl Parse for ParamFieldArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut out = Self::default();

        while !input.is_empty() {
            let key = input.parse::<Ident>()?;
            input.parse::<Token![=]>()?;

            if key == "default" {
                if out.default.is_some() {
                    return Err(Error::new(key.span(), "duplicate `default`"));
                }
                out.default = Some(input.parse::<Expr>()?);
            } else if key == "decl_id" {
                if out.decl_id.is_some() {
                    return Err(Error::new(key.span(), "duplicate `decl_id`"));
                }
                out.decl_id = Some(input.parse::<LitStr>()?);
            } else if key == "label" {
                if out.label.is_some() {
                    return Err(Error::new(key.span(), "duplicate `label`"));
                }
                out.label = Some(input.parse::<LitStr>()?);
            } else if key == "description" {
                if out.description.is_some() {
                    return Err(Error::new(key.span(), "duplicate `description`"));
                }
                out.description = Some(input.parse::<LitStr>()?);
            } else {
                return Err(Error::new(
                    key.span(),
                    "unsupported #[param(...)] argument (supported: default, decl_id, label, description)",
                ));
            }

            if input.is_empty() {
                break;
            }

            input.parse::<Token![,]>()?;
        }

        Ok(out)
    }
}

#[derive(Default)]
struct PotentialNodeFieldArgs {
    decl_id: Option<LitStr>,
}

impl Parse for PotentialNodeFieldArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut out = Self::default();

        while !input.is_empty() {
            let key = input.parse::<Ident>()?;
            input.parse::<Token![=]>()?;

            if key == "decl_id" {
                if out.decl_id.is_some() {
                    return Err(Error::new(key.span(), "duplicate `decl_id`"));
                }
                out.decl_id = Some(input.parse::<LitStr>()?);
            } else {
                return Err(Error::new(
                    key.span(),
                    "unsupported #[potential_node(...)] argument (supported: decl_id)",
                ));
            }

            if input.is_empty() {
                break;
            }

            input.parse::<Token![,]>()?;
        }

        Ok(out)
    }
}

struct ParamsDsl {
    items: Vec<ParamsDslItem>,
}

enum ParamsDslItem {
    Folder(ParamsDslFolder),
    Param(ParamsDslParam),
}

struct ParamsDslFolder {
    name: Ident,
    label: Option<LitStr>,
    items: Vec<ParamsDslItem>,
}

#[derive(Default)]
struct ParamsDslParamOptions {
    label: Option<LitStr>,
    description: Option<LitStr>,
    behaviour: Option<LitStr>,
}

struct ParamsDslParam {
    field: Ident,
    ty: Type,
    default: Option<Expr>,
    options: ParamsDslParamOptions,
}

struct ParamsDslOptionsOnly(ParamsDslParamOptions);

impl Parse for ParamsDslOptionsOnly {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Self(parse_params_options(input)?))
    }
}

impl Parse for ParamsDsl {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Self {
            items: parse_params_dsl_items(input)?,
        })
    }
}

fn parse_params_dsl_items(input: ParseStream) -> Result<Vec<ParamsDslItem>> {
    let mut items = Vec::new();

    while !input.is_empty() {
        let ident = input.parse::<Ident>()?;
        if ident == "folder" {
            let content;
            syn::parenthesized!(content in input);

            if content.is_empty() {
                return Err(Error::new(ident.span(), "folder(...) requires a name"));
            }

            let folder_name = content.parse::<Ident>()?;
            let mut folder_label = None::<LitStr>;

            while !content.is_empty() {
                content.parse::<Token![,]>()?;
                let key = content.parse::<Ident>()?;

                if key == "label" {
                    if folder_label.is_some() {
                        return Err(Error::new(key.span(), "duplicate folder label"));
                    }
                    content.parse::<Token![=]>()?;
                    folder_label = Some(content.parse::<LitStr>()?);
                } else if content.peek(Token![=]) {
                    content.parse::<Token![=]>()?;
                    let _: Expr = content.parse()?;
                }
            }

            let body;
            syn::braced!(body in input);
            let nested = parse_params_dsl_items(&body)?;

            if input.peek(Token![;]) {
                input.parse::<Token![;]>()?;
            }

            items.push(ParamsDslItem::Folder(ParamsDslFolder {
                name: folder_name,
                label: folder_label,
                items: nested,
            }));
            continue;
        }

        input.parse::<Token![:]>()?;
        let ty = input.parse::<Type>()?;

        let mut tail = Vec::<TokenTree>::new();
        while !input.peek(Token![;]) {
            tail.push(input.parse::<TokenTree>()?);
        }
        input.parse::<Token![;]>()?;

        let (default, options) = parse_param_tail(tail)?;

        items.push(ParamsDslItem::Param(ParamsDslParam {
            field: ident,
            ty,
            default,
            options,
        }));
    }

    Ok(items)
}

fn parse_params_options(input: ParseStream) -> Result<ParamsDslParamOptions> {
    let mut out = ParamsDslParamOptions::default();

    while !input.is_empty() {
        let key = input.parse::<Ident>()?;

        if input.peek(Token![=]) {
            input.parse::<Token![=]>()?;

            if key == "label" {
                if out.label.is_some() {
                    return Err(Error::new(key.span(), "duplicate `label` option"));
                }
                out.label = Some(input.parse::<LitStr>()?);
            } else if key == "description" {
                if out.description.is_some() {
                    return Err(Error::new(key.span(), "duplicate `description` option"));
                }
                out.description = Some(input.parse::<LitStr>()?);
            } else if key == "behavior" || key == "behaviour" {
                if out.behaviour.is_some() {
                    return Err(Error::new(key.span(), "duplicate `behavior` option"));
                }
                out.behaviour = Some(input.parse::<LitStr>()?);
            } else {
                let _: Expr = input.parse()?;
            }
        }

        if input.is_empty() {
            break;
        }

        input.parse::<Token![,]>()?;
    }

    Ok(out)
}

fn parse_param_tail(mut tail: Vec<TokenTree>) -> Result<(Option<Expr>, ParamsDslParamOptions)> {
    let mut options = ParamsDslParamOptions::default();

    if let Some(TokenTree::Group(group)) = tail.last() {
        if group.delimiter() == Delimiter::Parenthesis {
            if let Ok(parsed_options) = syn::parse2::<ParamsDslOptionsOnly>(group.stream()) {
                options = parsed_options.0;
                tail.pop();
            }
        }
    }

    tail.retain(|token| {
        !matches!(
            token,
            TokenTree::Group(group) if group.delimiter() == Delimiter::Bracket
        )
    });

    if tail.is_empty() {
        return Ok((None, options));
    }

    let Some(TokenTree::Punct(prefix)) = tail.first() else {
        return Err(Error::new(
            proc_macro2::Span::call_site(),
            "expected `=` before parameter default expression",
        ));
    };

    if prefix.as_char() != '=' {
        return Err(Error::new(
            prefix.span(),
            "expected `=` before parameter default expression",
        ));
    }

    let default_tokens: proc_macro2::TokenStream = tail.into_iter().skip(1).collect();
    if default_tokens.is_empty() {
        return Err(Error::new(
            proc_macro2::Span::call_site(),
            "missing parameter default expression after `=`",
        ));
    }

    let default_expr = syn::parse2::<Expr>(default_tokens)?;
    Ok((Some(default_expr), options))
}

#[derive(Default)]
struct ParamsParentChildren {
    folders: Vec<usize>,
    params: Vec<usize>,
}

struct ParamsFolderSpec {
    path: Vec<String>,
    decl_id: LitStr,
    label: LitStr,
}

enum ParamEventBehaviourSpec {
    Append,
    Coalesce,
}

struct ParamsParamSpec {
    field: Ident,
    ty: Type,
    path: Vec<String>,
    decl_id: LitStr,
    label: LitStr,
    description: Option<LitStr>,
    default: Option<Expr>,
    behaviour: Option<ParamEventBehaviourSpec>,
}

#[derive(Default)]
struct ParamsPlan {
    folders: Vec<ParamsFolderSpec>,
    params: Vec<ParamsParamSpec>,
    children_by_parent: BTreeMap<String, ParamsParentChildren>,
    max_depth: u32,
}

fn build_params_plan(dsl: &ParamsDsl) -> Result<ParamsPlan> {
    let mut plan = ParamsPlan::default();
    push_params_items_into_plan(&dsl.items, &[], &mut plan)?;
    Ok(plan)
}

fn push_params_items_into_plan(items: &[ParamsDslItem], parent_path: &[String], plan: &mut ParamsPlan) -> Result<()> {
    let parent_key = join_decl_path(parent_path);
    for item in items {
        match item {
            ParamsDslItem::Folder(folder) => {
                let mut path = parent_path.to_vec();
                path.push(folder.name.to_string());
                let decl_id_str = join_decl_path(&path);
                let decl_id_lit = LitStr::new(&decl_id_str, folder.name.span());
                let label_lit = folder
                    .label
                    .clone()
                    .unwrap_or_else(|| LitStr::new(&folder.name.to_string(), folder.name.span()));

                let folder_index = plan.folders.len();
                plan.folders.push(ParamsFolderSpec {
                    path: path.clone(),
                    decl_id: decl_id_lit,
                    label: label_lit,
                });
                plan.children_by_parent
                    .entry(parent_key.clone())
                    .or_default()
                    .folders
                    .push(folder_index);

                plan.max_depth = plan.max_depth.max(path.len() as u32);
                push_params_items_into_plan(&folder.items, &path, plan)?;
            }
            ParamsDslItem::Param(param) => {
                let mut path = parent_path.to_vec();
                path.push(param.field.to_string());
                let decl_id_str = join_decl_path(&path);
                let decl_id_lit = LitStr::new(&decl_id_str, param.field.span());
                let label_lit = param
                    .options
                    .label
                    .clone()
                    .unwrap_or_else(|| LitStr::new(&param.field.to_string(), param.field.span()));

                let behaviour = if let Some(value) = param.options.behaviour.clone() {
                    match value.value().to_ascii_lowercase().as_str() {
                        "append" => Some(ParamEventBehaviourSpec::Append),
                        "coalesce" => Some(ParamEventBehaviourSpec::Coalesce),
                        _ => {
                            return Err(Error::new(
                                value.span(),
                                "unsupported `behavior`; expected \"Append\" or \"Coalesce\"",
                            ));
                        }
                    }
                } else {
                    None
                };

                let param_index = plan.params.len();
                plan.params.push(ParamsParamSpec {
                    field: param.field.clone(),
                    ty: param.ty.clone(),
                    path: path.clone(),
                    decl_id: decl_id_lit,
                    label: label_lit,
                    description: param.options.description.clone(),
                    default: param.default.clone(),
                    behaviour,
                });
                plan.children_by_parent
                    .entry(parent_key.clone())
                    .or_default()
                    .params
                    .push(param_index);

                plan.max_depth = plan.max_depth.max(path.len() as u32);
            }
        }
    }

    Ok(())
}

fn join_decl_path(path: &[String]) -> String {
    path.join("/")
}

#[proc_macro_attribute]
pub fn node(attr: TokenStream, item: TokenStream) -> TokenStream {
    let NodeAttr { type_name, via } = parse_macro_input!(attr as NodeAttr);
    let input = parse_macro_input!(item as Item);

    match input {
        Item::Struct(input) => expand_struct(type_name, via, input).into(),
        Item::Impl(input) => expand_impl(type_name, via, input).into(),
        other => Error::new_spanned(
            other,
            "#[node] supports only structs and `impl Node for ...` blocks",
        )
        .to_compile_error()
        .into(),
    }
}

#[proc_macro_attribute]
pub fn update(attr: TokenStream, item: TokenStream) -> TokenStream {
    let UpdateAttr { rate_hz } = parse_macro_input!(attr as UpdateAttr);
    let input = parse_macro_input!(item as Item);

    let rate = match rate_hz.base10_parse::<u32>() {
        Ok(rate) => rate,
        Err(err) => {
            return Error::new(rate_hz.span(), format!("invalid update rate: {err}"))
                .to_compile_error()
                .into();
        }
    };

    if rate == 0 {
        return Error::new(rate_hz.span(), "update rate must be greater than zero")
            .to_compile_error()
            .into();
    }

    match input {
        Item::Impl(mut input) => {
            let Some((_, trait_path, _)) = &input.trait_ else {
                return Error::new_spanned(
                    input,
                    "#[update(...)] requires a trait impl: `impl Node for Type`",
                )
                .to_compile_error()
                .into();
            };

            let is_node_impl = trait_path
                .segments
                .last()
                .is_some_and(|seg| seg.ident == "Node");
            if !is_node_impl {
                return Error::new_spanned(
                    trait_path,
                    "#[update(...)] can only be used with `Node` trait impls",
                )
                .to_compile_error()
                .into();
            }

            if has_method(&input, "execution_rule") {
                return Error::new_spanned(
                    input,
                    "impl already defines `execution_rule`; remove #[update(...)] or the method",
                )
                .to_compile_error()
                .into();
            }

            input.items.push(parse_quote! {
                fn execution_rule(&self) -> golden_core::engine::NodeExecutionRule {
                    golden_core::engine::NodeExecutionRule::periodic(#rate)
                }
            });

            quote!(#input).into()
        }
        other => Error::new_spanned(
            other,
            "#[update(...)] supports only `impl Node for ...` blocks",
        )
        .to_compile_error()
        .into(),
    }
}

fn expand_struct(
    type_name: Option<LitStr>,
    via: Option<DelegatePath>,
    mut input: ItemStruct,
) -> proc_macro2::TokenStream {
    if via.is_some() {
        return Error::new_spanned(
            input,
            "`via = ...` is only supported on `impl Node for ...` blocks",
        )
        .to_compile_error();
    }

    let struct_name = input.ident.clone();
    let resolved_type_name = type_name.unwrap_or_else(|| make_type_name_literal(&struct_name.to_string()));
    let generics = input.generics.clone();
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let fields = match &mut input.fields {
        Fields::Named(named) => &mut named.named,
        _ => {
            return Error::new_spanned(
                input,
                "#[node(\"...\")] supports only structs with named fields",
            )
            .to_compile_error()
            .into();
        }
    };

    let has_node_data = fields
        .iter()
        .any(|f| f.ident.as_ref().is_some_and(|ident| ident == "node_data"));
    if !has_node_data {
        fields.insert(0, parse_quote!(node_data: golden_core::node::NodeData));
    }

    let mut ctor_fields = Vec::<(Ident, Type)>::new();
    let mut ctor_inits = Vec::<proc_macro2::TokenStream>::new();
    let mut generated_init_statements = Vec::<proc_macro2::TokenStream>::new();
    let mut child_added_decl_statements = Vec::<proc_macro2::TokenStream>::new();
    let mut child_replaced_decl_statements = Vec::<proc_macro2::TokenStream>::new();
    let mut child_removed_statements = Vec::<proc_macro2::TokenStream>::new();

    for field in fields.iter_mut() {
        let Some(field_ident) = field.ident.clone() else {
            continue;
        };

        if field_ident == "node_data" {
            continue;
        }

        let (param_attr, potential_attr) = take_handle_attrs(field);
        if param_attr.is_some() && potential_attr.is_some() {
            return Error::new_spanned(
                field,
                "field cannot have both #[param(...)] and #[potential_node(...)]",
            )
            .to_compile_error();
        }

        if let Some(param_attr) = param_attr {
            let args = match param_attr.parse_args::<ParamFieldArgs>() {
                Ok(args) => args,
                Err(err) => return err.to_compile_error(),
            };

            let Some(default_expr) = args.default else {
                return Error::new_spanned(
                    param_attr,
                    "#[param(...)] requires `default = ...`",
                )
                .to_compile_error();
            };

            let Some(param_value_ty) = extract_handle_inner_type(&field.ty, "ParameterHandle") else {
                return Error::new_spanned(
                    &field.ty,
                    "#[param(...)] requires field type ParameterHandle<T>",
                )
                .to_compile_error();
            };

            let decl_id_lit = args.decl_id.unwrap_or_else(|| LitStr::new(&field_ident.to_string(), field_ident.span()));
            let label_lit = args.label.unwrap_or_else(|| LitStr::new(&field_ident.to_string(), field_ident.span()));
            let set_description = args.description.map(|description_lit| {
                quote! {
                    golden_core::node::Node::node_data_mut(&mut __param_node).meta.description =
                        Some(::std::string::String::from(#description_lit));
                }
            });

            ctor_inits.push(quote! {
                #field_ident: golden_core::node::ParameterHandle::<#param_value_ty>::new(
                    golden_core::node::NodeId(0),
                    #default_expr
                )
            });

            generated_init_statements.push(quote! {
                {
                    let mut __param_node = golden_core::parameter::Parameter::new(
                        #label_lit,
                        <#param_value_ty as golden_core::node::ParameterValueType>::to_param_value(
                            self.#field_ident.get_cached().clone()
                        ),
                        self.#field_ident.change_check().clone(),
                    );
                    __param_node.event_behaviour = self.#field_ident.event_behaviour();
                    golden_core::node::Node::node_data_mut(&mut __param_node).meta.decl_id =
                        golden_core::node::DeclId(::std::string::String::from(#decl_id_lit));
                    #set_description
                    self.add_child(ctx, __param_node, None);
                }
            });

            child_added_decl_statements.push(quote! {
                if parent == self.id() && decl_id.0 == #decl_id_lit {
                    self.#field_ident.set_node_id(child);
                }
            });

            child_replaced_decl_statements.push(quote! {
                if parent == self.id() && decl_id.0 == #decl_id_lit {
                    self.#field_ident.set_node_id(new);
                }
            });

            child_removed_statements.push(quote! {
                if parent == self.id() && self.#field_ident.id() == child {
                    self.#field_ident.clear_node_id();
                }
            });

            continue;
        }

        if let Some(potential_attr) = potential_attr {
            let args = match potential_attr.parse_args::<PotentialNodeFieldArgs>() {
                Ok(args) => args,
                Err(err) => return err.to_compile_error(),
            };

            if !is_named_type(&field.ty, "PotentialNodeHandle") {
                return Error::new_spanned(
                    &field.ty,
                    "#[potential_node(...)] requires field type PotentialNodeHandle",
                )
                .to_compile_error();
            }

            let decl_id_lit = args.decl_id.unwrap_or_else(|| LitStr::new(&field_ident.to_string(), field_ident.span()));

            ctor_inits.push(quote! {
                #field_ident: golden_core::node::PotentialNodeHandle::new(
                    golden_core::node::NodeId(0),
                    #decl_id_lit
                )
            });

            generated_init_statements.push(quote! {
                self.#field_ident.set_parent(self.id());
            });

            child_added_decl_statements.push(quote! {
                let _ = self.#field_ident.reconcile_child_added(parent, child, decl_id);
            });

            child_replaced_decl_statements.push(quote! {
                let _ = self.#field_ident.reconcile_child_replaced(parent, old, new, decl_id);
            });

            child_removed_statements.push(quote! {
                let _ = self.#field_ident.reconcile_child_removed(parent, child);
            });

            continue;
        }

        ctor_fields.push((field_ident.clone(), field.ty.clone()));
        ctor_inits.push(quote! { #field_ident });
    }

    let ctor_args = ctor_fields.iter().map(|(ident, ty)| quote!(#ident: #ty));

    quote! {
        #input

        impl #impl_generics #struct_name #ty_generics #where_clause {
            pub fn new(label: impl Into<String> #(, #ctor_args)*) -> Self {
                Self {
                    node_data: golden_core::node::NodeData::new(label.into()),
                    #(#ctor_inits),*
                }
            }
        }

        impl #impl_generics golden_core::node::Node for #struct_name #ty_generics #where_clause {
            fn node_data(&self) -> &golden_core::node::NodeData {
                &self.node_data
            }

            fn node_data_mut(&mut self) -> &mut golden_core::node::NodeData {
                &mut self.node_data
            }

            fn get_type(&self) -> &str {
                #resolved_type_name
            }

            fn init(&mut self, ctx: &mut golden_core::process_ctx::ProcessCtx) {
                #(#generated_init_statements)*
            }

            fn on_child_added_decl(
                &mut self,
                _ctx: &mut golden_core::process_ctx::ProcessCtx,
                parent: golden_core::node::NodeId,
                child: golden_core::node::NodeId,
                decl_id: &golden_core::node::DeclId,
            ) {
                #(#child_added_decl_statements)*
            }

            fn on_child_replaced_decl(
                &mut self,
                _ctx: &mut golden_core::process_ctx::ProcessCtx,
                parent: golden_core::node::NodeId,
                old: golden_core::node::NodeId,
                new: golden_core::node::NodeId,
                decl_id: &golden_core::node::DeclId,
            ) {
                #(#child_replaced_decl_statements)*
            }

            fn on_child_removed(
                &mut self,
                _ctx: &mut golden_core::process_ctx::ProcessCtx,
                parent: golden_core::node::NodeId,
                child: golden_core::node::NodeId,
            ) {
                #(#child_removed_statements)*
            }
        }
    }
}

fn expand_impl(
    type_name: Option<LitStr>,
    via: Option<DelegatePath>,
    mut input: ItemImpl,
) -> proc_macro2::TokenStream {
    let Some((_, trait_path, _)) = &input.trait_ else {
        return Error::new_spanned(
            input,
            "#[node] on impl requires a trait impl: `impl Node for Type`",
        )
        .to_compile_error();
    };

    let is_node_impl = trait_path.segments.last().is_some_and(|seg| seg.ident == "Node");
    if !is_node_impl {
        return Error::new_spanned(
            trait_path,
            "#[node] on impl can only be used with `Node` trait",
        )
        .to_compile_error();
    }

    let node_data_body = if let Some(path) = via.as_ref() {
        let segments = &path.segments;
        quote! { golden_core::node::Node::node_data(&self.#(#segments).*) }
    } else {
        quote! { &self.node_data }
    };

    let node_data_mut_body = if let Some(path) = via.as_ref() {
        let segments = &path.segments;
        quote! { golden_core::node::Node::node_data_mut(&mut self.#(#segments).*) }
    } else {
        quote! { &mut self.node_data }
    };

    let mut params_dsl = None::<ParamsDsl>;
    let mut kept_items = Vec::with_capacity(input.items.len());
    for item in input.items.drain(..) {
        match item {
            ImplItem::Macro(macro_item) if is_params_macro(&macro_item) => {
                if params_dsl.is_some() {
                    return Error::new_spanned(macro_item, "only one params! { ... } block is supported per impl")
                        .to_compile_error();
                }
                let parsed = match syn::parse2::<ParamsDsl>(macro_item.mac.tokens.clone()) {
                    Ok(parsed) => parsed,
                    Err(err) => return err.to_compile_error(),
                };
                params_dsl = Some(parsed);
            }
            other => kept_items.push(other),
        }
    }
    input.items = kept_items;

    if !has_method(&input, "node_data") {
        input.items.push(parse_quote! {
            fn node_data(&self) -> &golden_core::node::NodeData {
                #node_data_body
            }
        });
    }

    if !has_method(&input, "node_data_mut") {
        input.items.push(parse_quote! {
            fn node_data_mut(&mut self) -> &mut golden_core::node::NodeData {
                #node_data_mut_body
            }
        });
    }

    if !has_method(&input, "get_type") {
        let resolved_type_name = match type_name {
            Some(type_name) => type_name,
            None => match infer_type_name_from_impl(&input) {
                Ok(type_name) => type_name,
                Err(err) => return err.to_compile_error(),
            },
        };
        input.items.push(parse_quote! {
            fn get_type(&self) -> &str {
                #resolved_type_name
            }
        });
    }

    if let Some(params_dsl) = params_dsl {
        let plan = match build_params_plan(&params_dsl) {
            Ok(plan) => plan,
            Err(err) => return err.to_compile_error(),
        };
        if let Err(err) = append_params_methods_to_impl(&mut input, &plan) {
            return err.to_compile_error();
        }
    }

    quote! {
        #input
    }
}

fn is_params_macro(item: &syn::ImplItemMacro) -> bool {
    item.mac
        .path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "params")
}

fn append_params_methods_to_impl(input: &mut ItemImpl, plan: &ParamsPlan) -> Result<()> {
    for method_name in [
        "init",
        "child_event_interest_depth",
        "on_child_added_decl",
        "on_child_replaced_decl",
        "on_child_removed",
    ] {
        if has_method(input, method_name) {
            return Err(Error::new_spanned(
                &*input,
                format!(
                    "params! generates `{method_name}`; remove the manual method or the params! block"
                ),
            ));
        }
    }

    let root_materialize = materialize_children_tokens(plan, "", quote!(self.id()));
    let max_depth = plan.max_depth.max(1);

    let folder_added_blocks = plan.folders.iter().map(|folder| {
        let decl_id_lit = &folder.decl_id;
        let folder_key = join_decl_path(&folder.path);
        let materialize = materialize_children_tokens(plan, &folder_key, quote!(child));
        quote! {
            if decl_id.0 == #decl_id_lit {
                #(#materialize)*
            }
        }
    });

    let folder_replaced_blocks = plan.folders.iter().map(|folder| {
        let decl_id_lit = &folder.decl_id;
        let folder_key = join_decl_path(&folder.path);
        let materialize = materialize_children_tokens(plan, &folder_key, quote!(new));
        quote! {
            if decl_id.0 == #decl_id_lit {
                #(#materialize)*
            }
        }
    });

    let param_added_bindings = plan.params.iter().map(|param| {
        let decl_id_lit = &param.decl_id;
        let field_ident = &param.field;
        quote! {
            if decl_id.0 == #decl_id_lit {
                self.#field_ident.set_node_id(child);
            }
        }
    });

    let param_replaced_bindings = plan.params.iter().map(|param| {
        let decl_id_lit = &param.decl_id;
        let field_ident = &param.field;
        quote! {
            if decl_id.0 == #decl_id_lit {
                self.#field_ident.set_node_id(new);
            }
        }
    });

    let param_removed_bindings = plan.params.iter().map(|param| {
        let field_ident = &param.field;
        quote! {
            if self.#field_ident.id() == child {
                self.#field_ident.clear_node_id();
            }
        }
    });

    input.items.push(parse_quote! {
        fn init(&mut self, ctx: &mut golden_core::process_ctx::ProcessCtx) {
            #(#root_materialize)*
        }
    });

    input.items.push(parse_quote! {
        fn child_event_interest_depth(&self, _event: &golden_core::events::Event) -> u32 {
            #max_depth
        }
    });

    input.items.push(parse_quote! {
        fn on_child_added_decl(
            &mut self,
            ctx: &mut golden_core::process_ctx::ProcessCtx,
            _parent: golden_core::node::NodeId,
            child: golden_core::node::NodeId,
            decl_id: &golden_core::node::DeclId,
        ) {
            #(#folder_added_blocks)*
            #(#param_added_bindings)*
        }
    });

    input.items.push(parse_quote! {
        fn on_child_replaced_decl(
            &mut self,
            ctx: &mut golden_core::process_ctx::ProcessCtx,
            _parent: golden_core::node::NodeId,
            _old: golden_core::node::NodeId,
            new: golden_core::node::NodeId,
            decl_id: &golden_core::node::DeclId,
        ) {
            #(#folder_replaced_blocks)*
            #(#param_replaced_bindings)*
        }
    });

    input.items.push(parse_quote! {
        fn on_child_removed(
            &mut self,
            _ctx: &mut golden_core::process_ctx::ProcessCtx,
            _parent: golden_core::node::NodeId,
            child: golden_core::node::NodeId,
        ) {
            #(#param_removed_bindings)*
        }
    });

    Ok(())
}

fn materialize_children_tokens(
    plan: &ParamsPlan,
    parent_key: &str,
    parent_expr: proc_macro2::TokenStream,
) -> Vec<proc_macro2::TokenStream> {
    let mut out = Vec::new();
    let Some(children) = plan.children_by_parent.get(parent_key) else {
        return out;
    };

    for folder_index in &children.folders {
        let folder = &plan.folders[*folder_index];
        let label_lit = &folder.label;
        let decl_id_lit = &folder.decl_id;
        let guard = folder_materialization_guard(plan, *folder_index);
        out.push(quote! {
            if #guard {
                let mut __folder_node = golden_core::node::Folder::new(#label_lit);
                golden_core::node::Node::node_data_mut(&mut __folder_node).meta.decl_id =
                    golden_core::node::DeclId(::std::string::String::from(#decl_id_lit));
                ctx.add_child(#parent_expr, __folder_node, None);
            }
        });
    }

    for param_index in &children.params {
        let param = &plan.params[*param_index];
        let field_ident = &param.field;
        let ty = &param.ty;
        let label_lit = &param.label;
        let decl_id_lit = &param.decl_id;
        let set_description = param.description.as_ref().map(|description_lit| {
            quote! {
                golden_core::node::Node::node_data_mut(&mut __param_node).meta.description =
                    Some(::std::string::String::from(#description_lit));
            }
        });

        let set_default = param.default.as_ref().map(|default_expr| {
            quote! {
                self.#field_ident.set_cached((#default_expr).into());
            }
        });

        let set_behaviour = match param.behaviour {
            Some(ParamEventBehaviourSpec::Append) => Some(quote! {
                self.#field_ident.set_event_behaviour(golden_core::parameter::ParameterEventBehaviour::Append);
            }),
            Some(ParamEventBehaviourSpec::Coalesce) => Some(quote! {
                self.#field_ident.set_event_behaviour(golden_core::parameter::ParameterEventBehaviour::Coalesce);
            }),
            None => None,
        };

        out.push(quote! {
            if !self.#field_ident.is_bound() {
                let _: &golden_core::node::ParameterHandle<#ty> = &self.#field_ident;
                #set_default
                #set_behaviour
                let mut __param_node = golden_core::parameter::Parameter::new(
                    #label_lit,
                    <#ty as golden_core::node::ParameterValueType>::to_param_value(
                        self.#field_ident.get_cached().clone()
                    ),
                    self.#field_ident.change_check().clone(),
                );
                __param_node.event_behaviour = self.#field_ident.event_behaviour();
                golden_core::node::Node::node_data_mut(&mut __param_node).meta.decl_id =
                    golden_core::node::DeclId(::std::string::String::from(#decl_id_lit));
                #set_description
                ctx.add_child(#parent_expr, __param_node, None);
            }
        });
    }

    out
}

fn folder_materialization_guard(plan: &ParamsPlan, folder_index: usize) -> proc_macro2::TokenStream {
    let folder = &plan.folders[folder_index];
    let descendant_params = plan
        .params
        .iter()
        .filter(|param| param.path.len() > folder.path.len() && param.path.starts_with(&folder.path))
        .map(|param| param.field.clone())
        .collect::<Vec<_>>();

    if descendant_params.is_empty() {
        quote!(true)
    } else {
        quote!(!(#(self.#descendant_params.is_bound())||*))
    }
}

fn take_handle_attrs(field: &mut Field) -> (Option<syn::Attribute>, Option<syn::Attribute>) {
    let mut param_attr = None;
    let mut potential_attr = None;
    let mut keep = Vec::with_capacity(field.attrs.len());

    for attr in field.attrs.drain(..) {
        if attr.path().is_ident("param") {
            param_attr = Some(attr);
        } else if attr.path().is_ident("potential_node") {
            potential_attr = Some(attr);
        } else {
            keep.push(attr);
        }
    }

    field.attrs = keep;
    (param_attr, potential_attr)
}

fn is_named_type(ty: &Type, ident: &str) -> bool {
    let Type::Path(path) = ty else {
        return false;
    };

    path.path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == ident)
}

fn extract_handle_inner_type(ty: &Type, handle_ident: &str) -> Option<Type> {
    let Type::Path(path) = ty else {
        return None;
    };

    let last = path.path.segments.last()?;
    if last.ident != handle_ident {
        return None;
    }

    let PathArguments::AngleBracketed(args) = &last.arguments else {
        return None;
    };

    let first = args.args.first()?;
    let GenericArgument::Type(inner) = first else {
        return None;
    };

    Some(inner.clone())
}

fn infer_type_name_from_impl(input: &ItemImpl) -> Result<LitStr> {
    let ident = match &*input.self_ty {
        Type::Path(path) if path.qself.is_none() => path.path.segments.last().map(|seg| seg.ident.to_string()),
        _ => None,
    };

    let Some(ident) = ident else {
        return Err(Error::new_spanned(
            &input.self_ty,
            "cannot infer node type name from impl target; use #[node(\"your_type\")]",
        ));
    };

    Ok(make_type_name_literal(&ident))
}

fn make_type_name_literal(type_ident: &str) -> LitStr {
    let snake = to_snake_case(type_ident);
    let trimmed = snake.strip_suffix("_node").unwrap_or(&snake);
    LitStr::new(trimmed, proc_macro2::Span::call_site())
}

fn to_snake_case(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::new();

    for i in 0..chars.len() {
        let c = chars[i];
        if c.is_ascii_uppercase() {
            if i > 0 {
                let prev = chars[i - 1];
                let next_is_lower = i + 1 < chars.len() && chars[i + 1].is_ascii_lowercase();
                if prev.is_ascii_lowercase() || prev.is_ascii_digit() || (prev.is_ascii_uppercase() && next_is_lower) {
                    out.push('_');
                }
            }
            out.push(c.to_ascii_lowercase());
        } else if c == '-' || c == ' ' {
            out.push('_');
        } else {
            out.push(c.to_ascii_lowercase());
        }
    }

    out
}

fn has_method(item_impl: &ItemImpl, name: &str) -> bool {
    item_impl.items.iter().any(|item| {
        matches!(
            item,
            ImplItem::Fn(function) if function.sig.ident == name
        )
    })
}
