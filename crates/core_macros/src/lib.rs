use std::collections::BTreeMap;

use proc_macro::TokenStream;
use proc_macro2::{Delimiter, Span, TokenTree};
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{
    Attribute, BinOp, Error, Expr, ExprArray, ExprBinary, ExprCall, ExprLit, ExprMethodCall, ExprPath, ExprUnary,
    Field, Fields, GenericArgument, Ident, ImplItem, Item, ItemImpl, ItemStruct, Lit, LitBool, LitInt, LitStr, Meta,
    PathArguments, Result, Token, Type, UnOp, parse_macro_input, parse_quote,
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

#[derive(Clone, Default)]
struct PresentationMetaFields {
    color: Option<Expr>,
    collapsed: Option<Expr>,
    warnings: Option<Expr>,
    show_child_warnings_max_depth: Option<Expr>,
    show_in_nested_inspector: Option<Expr>,
    show_in_inspector_content: Option<Expr>,
}

impl PresentationMetaFields {
    fn is_empty(&self) -> bool {
        self.color.is_none()
            && self.collapsed.is_none()
            && self.warnings.is_none()
            && self.show_child_warnings_max_depth.is_none()
            && self.show_in_nested_inspector.is_none()
            && self.show_in_inspector_content.is_none()
    }
}

fn try_parse_presentation_meta_field(
    key: &Ident,
    input: ParseStream,
    fields: &mut PresentationMetaFields,
) -> Result<bool> {
    let (slot, field_name): (&mut Option<Expr>, &str) = if key == "color" {
        (&mut fields.color, "color")
    } else if key == "collapsed" {
        (&mut fields.collapsed, "collapsed")
    } else if key == "warnings" {
        (&mut fields.warnings, "warnings")
    } else if key == "show_child_warnings_max_depth" || key == "showChildWarningsMaxDepth" {
        (
            &mut fields.show_child_warnings_max_depth,
            "show_child_warnings_max_depth",
        )
    } else if key == "show_in_nested_inspector" || key == "showInNestedInspector" {
        (&mut fields.show_in_nested_inspector, "show_in_nested_inspector")
    } else if key == "show_in_inspector_content" || key == "showInInspectorContent" {
        (&mut fields.show_in_inspector_content, "show_in_inspector_content")
    } else {
        return Ok(false);
    };

    if slot.is_some() {
        return Err(Error::new(
            key.span(),
            format!("duplicate presentation field `{field_name}`"),
        ));
    }

    input.parse::<Token![=]>()?;
    *slot = Some(input.parse::<Expr>()?);
    Ok(true)
}

struct NodeAttr {
    type_name: Option<LitStr>,
    ctor_meta_fields: BTreeMap<String, (Ident, Expr)>,
    presentation_fields: PresentationMetaFields,
    via: Option<DelegatePath>,
    impl_node: bool,
    from_struct: bool,
    scriptable: Option<ScriptableAttr>,
    contextualizable: Option<ContextualizableAttr>,
}

enum ScriptableAttr {
    Default,
    Expr(Box<Expr>),
}

enum ContextualizableAttr {
    Default,
    Expr(Box<Expr>),
}

impl Parse for NodeAttr {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut type_name = None;
        let mut ctor_meta_fields = BTreeMap::new();
        let mut presentation_fields = PresentationMetaFields::default();
        let mut via = None;
        let mut impl_node = false;
        let mut from_struct = false;
        let mut scriptable = None;
        let mut contextualizable = None;

        while !input.is_empty() {
            if input.peek(LitStr) {
                if type_name.is_some() {
                    return Err(Error::new(input.span(), "duplicate node type literal"));
                }
                type_name = Some(input.parse::<LitStr>()?);
            } else if input.peek(Ident) {
                let key = input.parse::<Ident>()?;
                if try_parse_presentation_meta_field(&key, input, &mut presentation_fields)? {
                } else if key == "via" {
                    if via.is_some() {
                        return Err(Error::new(key.span(), "duplicate `via` argument"));
                    }
                    input.parse::<Token![=]>()?;
                    via = Some(input.parse::<DelegatePath>()?);
                } else if key == "impl_node" {
                    if impl_node {
                        return Err(Error::new(key.span(), "duplicate `impl_node` argument"));
                    }
                    impl_node = true;
                } else if key == "from_struct" {
                    if from_struct {
                        return Err(Error::new(key.span(), "duplicate `from_struct` argument"));
                    }
                    from_struct = true;
                } else if key == "scriptable" {
                    if scriptable.is_some() {
                        return Err(Error::new(key.span(), "duplicate `scriptable` argument"));
                    }
                    if input.peek(Token![=]) {
                        input.parse::<Token![=]>()?;
                        scriptable = Some(ScriptableAttr::Expr(Box::new(input.parse::<Expr>()?)));
                    } else {
                        scriptable = Some(ScriptableAttr::Default);
                    }
                } else if key == "contextualizable" {
                    if contextualizable.is_some() {
                        return Err(Error::new(key.span(), "duplicate `contextualizable` argument"));
                    }
                    if input.peek(Token![=]) {
                        input.parse::<Token![=]>()?;
                        contextualizable = Some(ContextualizableAttr::Expr(Box::new(input.parse::<Expr>()?)));
                    } else {
                        contextualizable = Some(ContextualizableAttr::Default);
                    }
                } else {
                    let field_name = key.to_string();
                    if ctor_meta_fields.contains_key(&field_name) {
                        return Err(Error::new(key.span(), format!("duplicate meta field `{field_name}`")));
                    }
                    input.parse::<Token![=]>()?;
                    let expr = input.parse::<Expr>()?;
                    ctor_meta_fields.insert(field_name, (key, expr));
                }
            } else {
                return Err(Error::new(
                    input.span(),
                    "unexpected attribute arguments, expected string literal, constructor meta assignment like `label = \"...\"` or `can_be_disabled = false`, presentation sugar like `color = ...` or `collapsed = true`, `via = field.path`, `impl_node`, `from_struct`, `scriptable`, or `contextualizable`",
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

        Ok(Self {
            type_name,
            ctor_meta_fields,
            presentation_fields,
            via,
            impl_node,
            from_struct,
            scriptable,
            contextualizable,
        })
    }
}

struct ItemAttr {
    item_kind: Option<LitStr>,
    menu_path: Vec<LitStr>,
    node: NodeAttr,
}

struct NodeExpansion {
    node: NodeAttr,
    item_kind: Option<LitStr>,
    item_menu_path: Vec<LitStr>,
}

impl Parse for ItemAttr {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut item_kind = None;
        let mut menu_path = None::<Vec<LitStr>>;
        let mut type_name = None;
        let mut ctor_meta_fields = BTreeMap::new();
        let mut presentation_fields = PresentationMetaFields::default();
        let mut via = None;
        let mut impl_node = false;
        let mut from_struct = false;
        let mut scriptable = None;
        let mut contextualizable = None;

        while !input.is_empty() {
            if input.peek(LitStr) {
                let lit = input.parse::<LitStr>()?;
                if item_kind.is_none() {
                    item_kind = Some(lit);
                } else if type_name.is_none() {
                    type_name = Some(lit);
                } else {
                    return Err(Error::new(
                        input.span(),
                        "unexpected extra string literal; expected at most item kind and optional node type",
                    ));
                }
            } else if input.peek(Ident) {
                let key = input.parse::<Ident>()?;
                if try_parse_presentation_meta_field(&key, input, &mut presentation_fields)? {
                } else if key == "kind" {
                    if item_kind.is_some() {
                        return Err(Error::new(key.span(), "duplicate `kind` argument"));
                    }
                    input.parse::<Token![=]>()?;
                    item_kind = Some(input.parse::<LitStr>()?);
                } else if key == "node" || key == "node_type" {
                    if type_name.is_some() {
                        return Err(Error::new(key.span(), "duplicate node type argument"));
                    }
                    input.parse::<Token![=]>()?;
                    type_name = Some(input.parse::<LitStr>()?);
                } else if key == "menu_path" || key == "menuPath" {
                    if menu_path.is_some() {
                        return Err(Error::new(key.span(), "duplicate `menu_path` argument"));
                    }
                    input.parse::<Token![=]>()?;
                    menu_path = Some(parse_lit_str_array(input)?);
                } else if key == "via" {
                    if via.is_some() {
                        return Err(Error::new(key.span(), "duplicate `via` argument"));
                    }
                    input.parse::<Token![=]>()?;
                    via = Some(input.parse::<DelegatePath>()?);
                } else if key == "impl_node" {
                    if impl_node {
                        return Err(Error::new(key.span(), "duplicate `impl_node` argument"));
                    }
                    impl_node = true;
                } else if key == "from_struct" {
                    if from_struct {
                        return Err(Error::new(key.span(), "duplicate `from_struct` argument"));
                    }
                    from_struct = true;
                } else if key == "scriptable" {
                    if scriptable.is_some() {
                        return Err(Error::new(key.span(), "duplicate `scriptable` argument"));
                    }
                    if input.peek(Token![=]) {
                        input.parse::<Token![=]>()?;
                        scriptable = Some(ScriptableAttr::Expr(Box::new(input.parse::<Expr>()?)));
                    } else {
                        scriptable = Some(ScriptableAttr::Default);
                    }
                } else if key == "contextualizable" {
                    if contextualizable.is_some() {
                        return Err(Error::new(key.span(), "duplicate `contextualizable` argument"));
                    }
                    if input.peek(Token![=]) {
                        input.parse::<Token![=]>()?;
                        contextualizable = Some(ContextualizableAttr::Expr(Box::new(input.parse::<Expr>()?)));
                    } else {
                        contextualizable = Some(ContextualizableAttr::Default);
                    }
                } else {
                    let field_name = key.to_string();
                    if ctor_meta_fields.contains_key(&field_name) {
                        return Err(Error::new(key.span(), format!("duplicate meta field `{field_name}`")));
                    }
                    input.parse::<Token![=]>()?;
                    let expr = input.parse::<Expr>()?;
                    ctor_meta_fields.insert(field_name, (key, expr));
                }
            } else {
                return Err(Error::new(
                    input.span(),
                    "unexpected attribute arguments, expected item kind string literal or `kind = ...`, optional node type literal or `node = ...`, `menu_path = [\"...\"]`, constructor meta assignment like `label = \"...\"` or `can_be_disabled = false`, presentation sugar like `color = ...` or `collapsed = true`, plus `via = ...`, `impl_node`, `from_struct`, `scriptable`, `contextualizable`",
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

        Ok(Self {
            item_kind,
            menu_path: menu_path.unwrap_or_default(),
            node: NodeAttr {
                type_name,
                ctor_meta_fields,
                presentation_fields,
                via,
                impl_node,
                from_struct,
                scriptable,
                contextualizable,
            },
        })
    }
}

fn parse_lit_str_array(input: ParseStream) -> Result<Vec<LitStr>> {
    let content;
    syn::bracketed!(content in input);

    let mut values = Vec::new();
    while !content.is_empty() {
        values.push(content.parse::<LitStr>()?);
        if content.is_empty() {
            break;
        }
        content.parse::<Token![,]>()?;
        if content.is_empty() {
            break;
        }
    }

    Ok(values)
}

fn build_item_menu_path_tokens(item_menu_path: &[LitStr]) -> proc_macro2::TokenStream {
    if item_menu_path.is_empty() {
        quote! { ::std::vec::Vec::new() }
    } else {
        quote! {
            ::std::vec![#(::std::string::String::from(#item_menu_path)),*]
        }
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

struct StructDefaultsAttr {
    values: BTreeMap<String, (Ident, Expr)>,
}

impl Parse for StructDefaultsAttr {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut values = BTreeMap::new();

        while !input.is_empty() {
            let field = input.parse::<Ident>()?;
            let key = field.to_string();
            if values.contains_key(&key) {
                return Err(Error::new(field.span(), format!("duplicate default for field `{key}`")));
            }
            input.parse::<Token![=]>()?;
            let expr = input.parse::<Expr>()?;
            values.insert(key, (field, expr));

            if input.is_empty() {
                break;
            }

            input.parse::<Token![,]>()?;
            if input.is_empty() {
                break;
            }
        }

        Ok(Self { values })
    }
}

#[derive(Default)]
struct ParamFieldArgs {
    default: Option<Expr>,
    decl_id: Option<LitStr>,
    label: Option<LitStr>,
    description: Option<LitStr>,
    read_only: Option<Expr>,
    widget: Option<LitStr>,
    min: Option<Expr>,
    max: Option<Expr>,
    step: Option<Expr>,
    step_base: Option<Expr>,
    policy: Option<LitStr>,
    enum_options: Option<Expr>,
    file_allowed_types: Option<Expr>,
    file_allowed_extensions: Option<Expr>,
    dependency: Option<Expr>,
    default_callback: bool,
    callback: Option<Expr>,
}

impl Parse for ParamFieldArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut out = Self::default();

        while !input.is_empty() {
            let key = input.parse::<Ident>()?;
            if key == "default_callback" || key == "defaultCallback" {
                if out.default_callback {
                    return Err(Error::new(key.span(), "duplicate `default_callback`"));
                }
                if input.peek(Token![=]) {
                    input.parse::<Token![=]>()?;
                    out.default_callback = input.parse::<LitBool>()?.value;
                } else {
                    out.default_callback = true;
                }
            } else {
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
                } else if key == "read_only" || key == "readOnly" {
                    if out.read_only.is_some() {
                        return Err(Error::new(key.span(), "duplicate `read_only`"));
                    }
                    out.read_only = Some(input.parse::<Expr>()?);
                } else if key == "widget" {
                    if out.widget.is_some() {
                        return Err(Error::new(key.span(), "duplicate `widget`"));
                    }
                    out.widget = Some(input.parse::<LitStr>()?);
                } else if key == "min" {
                    if out.min.is_some() {
                        return Err(Error::new(key.span(), "duplicate `min`"));
                    }
                    out.min = Some(input.parse::<Expr>()?);
                } else if key == "max" {
                    if out.max.is_some() {
                        return Err(Error::new(key.span(), "duplicate `max`"));
                    }
                    out.max = Some(input.parse::<Expr>()?);
                } else if key == "step" {
                    if out.step.is_some() {
                        return Err(Error::new(key.span(), "duplicate `step`"));
                    }
                    out.step = Some(input.parse::<Expr>()?);
                } else if key == "step_base" || key == "stepBase" {
                    if out.step_base.is_some() {
                        return Err(Error::new(key.span(), "duplicate `step_base`"));
                    }
                    out.step_base = Some(input.parse::<Expr>()?);
                } else if key == "policy" || key == "constraint_policy" || key == "constraintPolicy" {
                    if out.policy.is_some() {
                        return Err(Error::new(key.span(), "duplicate `policy`"));
                    }
                    out.policy = Some(input.parse::<LitStr>()?);
                } else if key == "enum_options" || key == "enumOptions" {
                    if out.enum_options.is_some() {
                        return Err(Error::new(key.span(), "duplicate `enum_options`"));
                    }
                    out.enum_options = Some(input.parse::<Expr>()?);
                } else if key == "file_allowed_types"
                    || key == "fileAllowedTypes"
                    || key == "allowed_types"
                    || key == "allowedTypes"
                {
                    if out.file_allowed_types.is_some() {
                        return Err(Error::new(key.span(), "duplicate `file_allowed_types`"));
                    }
                    out.file_allowed_types = Some(input.parse::<Expr>()?);
                } else if key == "file_allowed_extensions"
                    || key == "fileAllowedExtensions"
                    || key == "allowed_extensions"
                    || key == "allowedExtensions"
                {
                    if out.file_allowed_extensions.is_some() {
                        return Err(Error::new(key.span(), "duplicate `file_allowed_extensions`"));
                    }
                    out.file_allowed_extensions = Some(input.parse::<Expr>()?);
                } else if key == "dependency" {
                    if out.dependency.is_some() {
                        return Err(Error::new(key.span(), "duplicate `dependency`"));
                    }
                    out.dependency = Some(input.parse::<Expr>()?);
                } else if key == "callback" {
                    if out.callback.is_some() {
                        return Err(Error::new(key.span(), "duplicate `callback`"));
                    }
                    out.callback = Some(input.parse::<Expr>()?);
                } else {
                    return Err(Error::new(
                        key.span(),
                        "unsupported #[param(...)] argument (supported: default, decl_id, label, description, read_only, widget, min, max, step, step_base, policy, enum_options, file_allowed_types, file_allowed_extensions, dependency, default_callback, callback)",
                    ));
                }
            }

            if input.is_empty() {
                break;
            }

            input.parse::<Token![,]>()?;
        }

        if out.default_callback && out.callback.is_some() {
            return Err(Error::new(
                input.span(),
                "cannot combine `default_callback` and `callback`; choose one callback style",
            ));
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

#[derive(Default)]
struct StateFieldArgs {
    default: Option<Expr>,
    persist: Option<bool>,
}

impl Parse for StateFieldArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut out = Self::default();

        while !input.is_empty() {
            let key = input.parse::<Ident>()?;
            if key == "persist" {
                if out.persist.is_some() {
                    return Err(Error::new(key.span(), "duplicate `persist`"));
                }
                if input.peek(Token![=]) {
                    input.parse::<Token![=]>()?;
                    out.persist = Some(input.parse::<LitBool>()?.value);
                } else {
                    out.persist = Some(true);
                }
            } else {
                input.parse::<Token![=]>()?;

                if key == "default" {
                    if out.default.is_some() {
                        return Err(Error::new(key.span(), "duplicate `default`"));
                    }
                    out.default = Some(input.parse::<Expr>()?);
                } else {
                    return Err(Error::new(
                        key.span(),
                        "unsupported #[state(...)] argument (supported: default, persist)",
                    ));
                }
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
    Folder(Box<ParamsDslFolder>),
    Param(Box<ParamsDslParam>),
    Node(Box<ParamsDslNode>),
    BaseChildren,
}

struct ParamsDslFolder {
    name: Ident,
    label: Option<LitStr>,
    description: Option<LitStr>,
    reuse: bool,
    meta: ParamsDslMetaOptions,
    items: Vec<ParamsDslItem>,
}

#[derive(Clone, Default)]
struct ParamsDslMetaOptions {
    short_name: Option<LitStr>,
    enabled: Option<Expr>,
    can_be_disabled: Option<Expr>,
    tags: Option<Expr>,
    semantics: Option<Expr>,
    presentation: Option<Expr>,
    presentation_fields: PresentationMetaFields,
}

#[derive(Default)]
struct ParamsDslParamOptions {
    label: Option<LitStr>,
    description: Option<LitStr>,
    read_only: Option<Expr>,
    widget: Option<LitStr>,
    dependency: Option<Expr>,
    meta: ParamsDslMetaOptions,
    behaviour: Option<LitStr>,
    min: Option<Expr>,
    max: Option<Expr>,
    step: Option<Expr>,
    step_base: Option<Expr>,
    policy: Option<LitStr>,
    enum_options: Option<Expr>,
    enum_default: Option<LitStr>,
    file_allowed_types: Option<Expr>,
    file_allowed_extensions: Option<Expr>,
    reference_root: Option<Expr>,
    reference_target_kind: Option<Expr>,
    reference_allowed_node_types: Option<Expr>,
    reference_allowed_parameter_types: Option<Expr>,
    reference_allow_projections: Option<Expr>,
    reference_custom_filter_key: Option<Expr>,
    reference_default_search_filter: Option<Expr>,
    default_callback: bool,
    callback: Option<Expr>,
}

struct ParamsDslParam {
    field: Ident,
    ty: Type,
    default: Option<Expr>,
    options: ParamsDslParamOptions,
}

#[derive(Default)]
struct ParamsDslNodeOptions {
    label: Option<LitStr>,
    description: Option<LitStr>,
    meta: ParamsDslMetaOptions,
}

struct ParamsDslNode {
    field: Ident,
    ty: Type,
    default: Expr,
    options: ParamsDslNodeOptions,
}

struct ParamsDslOptionsOnly(ParamsDslParamOptions);

impl Parse for ParamsDslOptionsOnly {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Self(parse_params_options(input)?))
    }
}

struct ParamsDslNodeOptionsOnly(ParamsDslNodeOptions);

impl Parse for ParamsDslNodeOptionsOnly {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Self(parse_node_options(input)?))
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
        if input.peek(syn::token::Bracket) {
            let content;
            syn::bracketed!(content in input);
            let placeholder = content.parse::<Ident>()?;
            if !content.is_empty() {
                return Err(Error::new(
                    placeholder.span(),
                    "base child placeholder expects exactly one identifier",
                ));
            }
            if placeholder != "base_children" {
                return Err(Error::new(
                    placeholder.span(),
                    "unsupported children placeholder; expected `[base_children]`",
                ));
            }
            if input.peek(Token![;]) {
                input.parse::<Token![;]>()?;
            }
            items.push(ParamsDslItem::BaseChildren);
            continue;
        }

        let ident = input.parse::<Ident>()?;
        if ident == "base_children" {
            return Err(Error::new(
                ident.span(),
                "use `[base_children]` to place composed children",
            ));
        }

        if ident == "folder" {
            let content;
            syn::parenthesized!(content in input);

            if content.is_empty() {
                return Err(Error::new(ident.span(), "folder(...) requires a name"));
            }

            let folder_name = content.parse::<Ident>()?;
            let mut folder_label = None::<LitStr>;
            let mut folder_description = None::<LitStr>;
            let mut folder_reuse = None::<bool>;
            let mut folder_meta = ParamsDslMetaOptions::default();

            while !content.is_empty() {
                content.parse::<Token![,]>()?;
                if content.is_empty() {
                    break;
                }
                let key = content.parse::<Ident>()?;

                if key == "label" {
                    if folder_label.is_some() {
                        return Err(Error::new(key.span(), "duplicate folder label"));
                    }
                    content.parse::<Token![=]>()?;
                    folder_label = Some(content.parse::<LitStr>()?);
                } else if key == "description" {
                    if folder_description.is_some() {
                        return Err(Error::new(key.span(), "duplicate folder description"));
                    }
                    content.parse::<Token![=]>()?;
                    folder_description = Some(content.parse::<LitStr>()?);
                } else if key == "reuse" {
                    if folder_reuse.is_some() {
                        return Err(Error::new(key.span(), "duplicate folder reuse flag"));
                    }
                    content.parse::<Token![=]>()?;
                    folder_reuse = Some(content.parse::<LitBool>()?.value);
                } else if key == "short_name" || key == "shortName" {
                    if folder_meta.short_name.is_some() {
                        return Err(Error::new(key.span(), "duplicate folder short_name"));
                    }
                    content.parse::<Token![=]>()?;
                    folder_meta.short_name = Some(content.parse::<LitStr>()?);
                } else if key == "enabled" {
                    if folder_meta.enabled.is_some() {
                        return Err(Error::new(key.span(), "duplicate folder enabled"));
                    }
                    content.parse::<Token![=]>()?;
                    folder_meta.enabled = Some(content.parse::<Expr>()?);
                } else if key == "can_be_disabled" || key == "canBeDisabled" {
                    if folder_meta.can_be_disabled.is_some() {
                        return Err(Error::new(key.span(), "duplicate folder can_be_disabled"));
                    }
                    content.parse::<Token![=]>()?;
                    folder_meta.can_be_disabled = Some(content.parse::<Expr>()?);
                } else if key == "tags" {
                    if folder_meta.tags.is_some() {
                        return Err(Error::new(key.span(), "duplicate folder tags"));
                    }
                    content.parse::<Token![=]>()?;
                    folder_meta.tags = Some(content.parse::<Expr>()?);
                } else if key == "semantics" {
                    if folder_meta.semantics.is_some() {
                        return Err(Error::new(key.span(), "duplicate folder semantics"));
                    }
                    content.parse::<Token![=]>()?;
                    folder_meta.semantics = Some(content.parse::<Expr>()?);
                } else if key == "presentation" {
                    if folder_meta.presentation.is_some() {
                        return Err(Error::new(key.span(), "duplicate folder presentation"));
                    }
                    content.parse::<Token![=]>()?;
                    folder_meta.presentation = Some(content.parse::<Expr>()?);
                } else if try_parse_presentation_meta_field(&key, &content, &mut folder_meta.presentation_fields)? {
                } else {
                    return Err(Error::new(
                        key.span(),
                        "unsupported folder(...) argument (supported: label, description, reuse, short_name, enabled, can_be_disabled, tags, semantics, presentation, color, collapsed, warnings, show_child_warnings_max_depth, show_in_nested_inspector, show_in_inspector_content)",
                    ));
                }
            }

            let body;
            syn::braced!(body in input);
            let nested = parse_params_dsl_items(&body)?;

            if input.peek(Token![;]) {
                input.parse::<Token![;]>()?;
            }

            items.push(ParamsDslItem::Folder(Box::new(ParamsDslFolder {
                name: folder_name,
                label: folder_label,
                description: folder_description,
                reuse: folder_reuse.unwrap_or(true),
                meta: folder_meta,
                items: nested,
            })));
            continue;
        }

        if ident == "node" {
            let field = input.parse::<Ident>()?;
            input.parse::<Token![:]>()?;
            let ty = input.parse::<Type>()?;

            let mut tail = Vec::<TokenTree>::new();
            while !input.peek(Token![;]) {
                tail.push(input.parse::<TokenTree>()?);
            }
            input.parse::<Token![;]>()?;

            let (default, options) = parse_node_tail(tail)?;

            items.push(ParamsDslItem::Node(Box::new(ParamsDslNode {
                field,
                ty,
                default,
                options,
            })));
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

        items.push(ParamsDslItem::Param(Box::new(ParamsDslParam {
            field: ident,
            ty,
            default,
            options,
        })));
    }

    Ok(items)
}

fn parse_node_options(input: ParseStream) -> Result<ParamsDslNodeOptions> {
    let mut out = ParamsDslNodeOptions::default();

    while !input.is_empty() {
        let key = input.parse::<Ident>()?;
        if try_parse_presentation_meta_field(&key, input, &mut out.meta.presentation_fields)? {
        } else {
            if !input.peek(Token![=]) {
                return Err(Error::new(
                    key.span(),
                    "unsupported node child option flag; expected `key = value`",
                ));
            }

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
            } else if key == "short_name" || key == "shortName" {
                if out.meta.short_name.is_some() {
                    return Err(Error::new(key.span(), "duplicate `short_name` option"));
                }
                out.meta.short_name = Some(input.parse::<LitStr>()?);
            } else if key == "enabled" {
                if out.meta.enabled.is_some() {
                    return Err(Error::new(key.span(), "duplicate `enabled` option"));
                }
                out.meta.enabled = Some(input.parse::<Expr>()?);
            } else if key == "can_be_disabled" || key == "canBeDisabled" {
                if out.meta.can_be_disabled.is_some() {
                    return Err(Error::new(key.span(), "duplicate `can_be_disabled` option"));
                }
                out.meta.can_be_disabled = Some(input.parse::<Expr>()?);
            } else if key == "tags" {
                if out.meta.tags.is_some() {
                    return Err(Error::new(key.span(), "duplicate `tags` option"));
                }
                out.meta.tags = Some(input.parse::<Expr>()?);
            } else if key == "semantics" {
                if out.meta.semantics.is_some() {
                    return Err(Error::new(key.span(), "duplicate `semantics` option"));
                }
                out.meta.semantics = Some(input.parse::<Expr>()?);
            } else if key == "presentation" {
                if out.meta.presentation.is_some() {
                    return Err(Error::new(key.span(), "duplicate `presentation` option"));
                }
                out.meta.presentation = Some(input.parse::<Expr>()?);
            } else {
                return Err(Error::new(
                    key.span(),
                    "unsupported node child option (supported: label, description, short_name, enabled, can_be_disabled, tags, semantics, presentation, color, collapsed, warnings, show_child_warnings_max_depth, show_in_nested_inspector, show_in_inspector_content)",
                ));
            }
        }

        if input.is_empty() {
            break;
        }

        input.parse::<Token![,]>()?;
    }

    Ok(out)
}

fn parse_params_options(input: ParseStream) -> Result<ParamsDslParamOptions> {
    let mut out = ParamsDslParamOptions::default();

    while !input.is_empty() {
        let key = input.parse::<Ident>()?;

        if key == "default_callback" || key == "defaultCallback" {
            if out.default_callback {
                return Err(Error::new(key.span(), "duplicate `default_callback` option"));
            }
            if input.peek(Token![=]) {
                input.parse::<Token![=]>()?;
                out.default_callback = input.parse::<LitBool>()?.value;
            } else {
                out.default_callback = true;
            }
        } else if try_parse_presentation_meta_field(&key, input, &mut out.meta.presentation_fields)? {
        } else if input.peek(Token![=]) {
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
            } else if key == "read_only" || key == "readOnly" {
                if out.read_only.is_some() {
                    return Err(Error::new(key.span(), "duplicate `read_only` option"));
                }
                out.read_only = Some(input.parse::<Expr>()?);
            } else if key == "widget" {
                if out.widget.is_some() {
                    return Err(Error::new(key.span(), "duplicate `widget` option"));
                }
                out.widget = Some(input.parse::<LitStr>()?);
            } else if key == "dependency" {
                if out.dependency.is_some() {
                    return Err(Error::new(key.span(), "duplicate `dependency` option"));
                }
                out.dependency = Some(input.parse::<Expr>()?);
            } else if key == "short_name" || key == "shortName" {
                if out.meta.short_name.is_some() {
                    return Err(Error::new(key.span(), "duplicate `short_name` option"));
                }
                out.meta.short_name = Some(input.parse::<LitStr>()?);
            } else if key == "enabled" {
                if out.meta.enabled.is_some() {
                    return Err(Error::new(key.span(), "duplicate `enabled` option"));
                }
                out.meta.enabled = Some(input.parse::<Expr>()?);
            } else if key == "can_be_disabled" || key == "canBeDisabled" {
                if out.meta.can_be_disabled.is_some() {
                    return Err(Error::new(key.span(), "duplicate `can_be_disabled` option"));
                }
                out.meta.can_be_disabled = Some(input.parse::<Expr>()?);
            } else if key == "tags" {
                if out.meta.tags.is_some() {
                    return Err(Error::new(key.span(), "duplicate `tags` option"));
                }
                out.meta.tags = Some(input.parse::<Expr>()?);
            } else if key == "semantics" {
                if out.meta.semantics.is_some() {
                    return Err(Error::new(key.span(), "duplicate `semantics` option"));
                }
                out.meta.semantics = Some(input.parse::<Expr>()?);
            } else if key == "presentation" {
                if out.meta.presentation.is_some() {
                    return Err(Error::new(key.span(), "duplicate `presentation` option"));
                }
                out.meta.presentation = Some(input.parse::<Expr>()?);
            } else if key == "behavior" || key == "behaviour" {
                if out.behaviour.is_some() {
                    return Err(Error::new(key.span(), "duplicate `behavior` option"));
                }
                out.behaviour = Some(input.parse::<LitStr>()?);
            } else if key == "min" {
                if out.min.is_some() {
                    return Err(Error::new(key.span(), "duplicate `min` option"));
                }
                out.min = Some(input.parse::<Expr>()?);
            } else if key == "max" {
                if out.max.is_some() {
                    return Err(Error::new(key.span(), "duplicate `max` option"));
                }
                out.max = Some(input.parse::<Expr>()?);
            } else if key == "step" {
                if out.step.is_some() {
                    return Err(Error::new(key.span(), "duplicate `step` option"));
                }
                out.step = Some(input.parse::<Expr>()?);
            } else if key == "step_base" || key == "stepBase" {
                if out.step_base.is_some() {
                    return Err(Error::new(key.span(), "duplicate `step_base` option"));
                }
                out.step_base = Some(input.parse::<Expr>()?);
            } else if key == "policy" || key == "constraint_policy" || key == "constraintPolicy" {
                if out.policy.is_some() {
                    return Err(Error::new(key.span(), "duplicate `policy` option"));
                }
                out.policy = Some(input.parse::<LitStr>()?);
            } else if key == "enum_options" || key == "enumOptions" {
                if out.enum_options.is_some() {
                    return Err(Error::new(key.span(), "duplicate `enum_options` option"));
                }
                out.enum_options = Some(input.parse::<Expr>()?);
            } else if key == "enum_default" || key == "enumDefault" {
                if out.enum_default.is_some() {
                    return Err(Error::new(key.span(), "duplicate `enum_default` option"));
                }
                out.enum_default = Some(input.parse::<LitStr>()?);
            } else if key == "file_allowed_types"
                || key == "fileAllowedTypes"
                || key == "allowed_types"
                || key == "allowedTypes"
            {
                if out.file_allowed_types.is_some() {
                    return Err(Error::new(key.span(), "duplicate `file_allowed_types` option"));
                }
                out.file_allowed_types = Some(input.parse::<Expr>()?);
            } else if key == "file_allowed_extensions"
                || key == "fileAllowedExtensions"
                || key == "allowed_extensions"
                || key == "allowedExtensions"
            {
                if out.file_allowed_extensions.is_some() {
                    return Err(Error::new(key.span(), "duplicate `file_allowed_extensions` option"));
                }
                out.file_allowed_extensions = Some(input.parse::<Expr>()?);
            } else if key == "reference_root" || key == "referenceRoot" {
                if out.reference_root.is_some() {
                    return Err(Error::new(key.span(), "duplicate `reference_root` option"));
                }
                out.reference_root = Some(input.parse::<Expr>()?);
            } else if key == "reference_target_kind" || key == "referenceTargetKind" {
                if out.reference_target_kind.is_some() {
                    return Err(Error::new(key.span(), "duplicate `reference_target_kind` option"));
                }
                out.reference_target_kind = Some(input.parse::<Expr>()?);
            } else if key == "reference_allowed_node_types" || key == "referenceAllowedNodeTypes" {
                if out.reference_allowed_node_types.is_some() {
                    return Err(Error::new(
                        key.span(),
                        "duplicate `reference_allowed_node_types` option",
                    ));
                }
                out.reference_allowed_node_types = Some(input.parse::<Expr>()?);
            } else if key == "reference_allowed_parameter_types" || key == "referenceAllowedParameterTypes" {
                if out.reference_allowed_parameter_types.is_some() {
                    return Err(Error::new(
                        key.span(),
                        "duplicate `reference_allowed_parameter_types` option",
                    ));
                }
                out.reference_allowed_parameter_types = Some(input.parse::<Expr>()?);
            } else if key == "reference_allow_projections"
                || key == "referenceAllowProjections"
                || key == "reference_allow_projection"
                || key == "referenceAllowProjection"
            {
                if out.reference_allow_projections.is_some() {
                    return Err(Error::new(key.span(), "duplicate `reference_allow_projections` option"));
                }
                out.reference_allow_projections = Some(input.parse::<Expr>()?);
            } else if key == "reference_custom_filter_key" || key == "referenceCustomFilterKey" {
                if out.reference_custom_filter_key.is_some() {
                    return Err(Error::new(key.span(), "duplicate `reference_custom_filter_key` option"));
                }
                out.reference_custom_filter_key = Some(input.parse::<Expr>()?);
            } else if key == "reference_default_search_filter" || key == "referenceDefaultSearchFilter" {
                if out.reference_default_search_filter.is_some() {
                    return Err(Error::new(
                        key.span(),
                        "duplicate `reference_default_search_filter` option",
                    ));
                }
                out.reference_default_search_filter = Some(input.parse::<Expr>()?);
            } else if key == "callback" {
                if out.callback.is_some() {
                    return Err(Error::new(key.span(), "duplicate `callback` option"));
                }
                out.callback = Some(input.parse::<Expr>()?);
            } else {
                return Err(Error::new(
                    key.span(),
                    "unsupported parameter child option (supported: label, description, read_only, widget, dependency, short_name, enabled, can_be_disabled, tags, semantics, presentation, color, collapsed, warnings, show_child_warnings_max_depth, show_in_nested_inspector, show_in_inspector_content, behavior, min, max, step, step_base, policy, enum_options, enum_default, file_allowed_types, file_allowed_extensions, reference_root, reference_target_kind, reference_allowed_node_types, reference_allowed_parameter_types, reference_allow_projections, reference_custom_filter_key, reference_default_search_filter, default_callback, callback)",
                ));
            }
        } else {
            return Err(Error::new(
                key.span(),
                "unsupported flag option; expected `default_callback`",
            ));
        }

        if input.is_empty() {
            break;
        }

        input.parse::<Token![,]>()?;
    }

    if out.default_callback && out.callback.is_some() {
        return Err(Error::new(
            input.span(),
            "cannot combine `default_callback` and `callback`; choose one callback style",
        ));
    }

    Ok(out)
}

fn parse_param_tail(mut tail: Vec<TokenTree>) -> Result<(Option<Expr>, ParamsDslParamOptions)> {
    let mut options = ParamsDslParamOptions::default();

    if let Some(TokenTree::Group(group)) = tail.last()
        && group.delimiter() == Delimiter::Parenthesis
        && let Ok(parsed_options) = syn::parse2::<ParamsDslOptionsOnly>(group.stream())
    {
        options = parsed_options.0;
        tail.pop();
    }

    if let Some(TokenTree::Group(group)) = tail.last()
        && group.delimiter() == Delimiter::Bracket
        && let Some(range) = parse_param_range_group(group)?
    {
        if range.min.is_some() && options.min.is_some() {
            return Err(Error::new(
                group.span(),
                "duplicate `min`; provided by both `[...]` and options",
            ));
        }
        if range.max.is_some() && options.max.is_some() {
            return Err(Error::new(
                group.span(),
                "duplicate `max`; provided by both `[...]` and options",
            ));
        }
        options.min = options.min.or(range.min);
        options.max = options.max.or(range.max);
        tail.pop();
    }

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

fn parse_node_tail(mut tail: Vec<TokenTree>) -> Result<(Expr, ParamsDslNodeOptions)> {
    let mut options = ParamsDslNodeOptions::default();

    if let Some(TokenTree::Group(group)) = tail.last()
        && group.delimiter() == Delimiter::Parenthesis
        && let Ok(parsed_options) = syn::parse2::<ParamsDslNodeOptionsOnly>(group.stream())
    {
        options = parsed_options.0;
        tail.pop();
    }

    if tail.is_empty() {
        return Err(Error::new(
            proc_macro2::Span::call_site(),
            "node child declaration requires `= ...` to construct the child node",
        ));
    }

    let Some(TokenTree::Punct(prefix)) = tail.first() else {
        return Err(Error::new(
            proc_macro2::Span::call_site(),
            "expected `=` before node child expression",
        ));
    };

    if prefix.as_char() != '=' {
        return Err(Error::new(prefix.span(), "expected `=` before node child expression"));
    }

    let default_tokens: proc_macro2::TokenStream = tail.into_iter().skip(1).collect();
    if default_tokens.is_empty() {
        return Err(Error::new(
            proc_macro2::Span::call_site(),
            "missing node child expression after `=`",
        ));
    }

    let default_expr = syn::parse2::<Expr>(default_tokens)?;
    Ok((default_expr, options))
}

struct ParamsDslRange {
    min: Option<Expr>,
    max: Option<Expr>,
}

fn parse_param_range_group(group: &proc_macro2::Group) -> Result<Option<ParamsDslRange>> {
    let tokens: Vec<TokenTree> = group.stream().into_iter().collect();
    if tokens.is_empty() {
        return Ok(None);
    }

    let mut separator = None::<(usize, usize)>;
    let mut i = 0usize;

    while i < tokens.len() {
        let two_dots = matches!(tokens.get(i), Some(TokenTree::Punct(p)) if p.as_char() == '.')
            && matches!(tokens.get(i + 1), Some(TokenTree::Punct(p)) if p.as_char() == '.');
        if two_dots {
            if separator.is_some() {
                return Err(Error::new(
                    group.span(),
                    "invalid `[...]` range: expected a single `..` separator",
                ));
            }
            if matches!(tokens.get(i + 2), Some(TokenTree::Punct(p)) if p.as_char() == '=') {
                separator = Some((i, 3));
                i += 3;
            } else {
                separator = Some((i, 2));
                i += 2;
            }
            continue;
        }
        i += 1;
    }

    let Some((separator_start, separator_width)) = separator else {
        return Ok(None);
    };

    let left_tokens: proc_macro2::TokenStream = tokens.iter().take(separator_start).cloned().collect();
    let right_tokens: proc_macro2::TokenStream =
        tokens.iter().skip(separator_start + separator_width).cloned().collect();

    let min = if left_tokens.is_empty() {
        None
    } else {
        Some(
            syn::parse2::<Expr>(left_tokens)
                .map_err(|err| Error::new(group.span(), format!("invalid min expression in `[...]`: {err}")))?,
        )
    };

    let max = if right_tokens.is_empty() {
        None
    } else {
        Some(
            syn::parse2::<Expr>(right_tokens)
                .map_err(|err| Error::new(group.span(), format!("invalid max expression in `[...]`: {err}")))?,
        )
    };

    if min.is_none() && max.is_none() {
        return Err(Error::new(
            group.span(),
            "invalid `[...]` range: expected at least one bound",
        ));
    }

    Ok(Some(ParamsDslRange { min, max }))
}

#[derive(Clone)]
struct SimpleEnumOptionSpec {
    variant_id: LitStr,
    label: LitStr,
    is_default: bool,
}

#[derive(Clone)]
struct SimpleEnumOptionsSpec {
    options: Vec<SimpleEnumOptionSpec>,
}

fn parse_simple_enum_options_expr(expr: &Expr) -> Result<Option<SimpleEnumOptionsSpec>> {
    let Expr::Array(ExprArray { elems, .. }) = expr else {
        return Ok(None);
    };

    let mut options = Vec::<SimpleEnumOptionSpec>::new();
    for elem in elems {
        let Expr::Lit(ExprLit {
            lit: Lit::Str(raw_lit), ..
        }) = elem
        else {
            return Ok(None);
        };

        let (variant_id, is_default) = parse_simple_enum_literal(raw_lit)?;
        let label = enum_label_from_variant_id(&variant_id);
        options.push(SimpleEnumOptionSpec {
            variant_id: LitStr::new(&variant_id, raw_lit.span()),
            label: LitStr::new(&label, raw_lit.span()),
            is_default,
        });
    }

    Ok(Some(SimpleEnumOptionsSpec { options }))
}

fn parse_simple_enum_literal(raw_lit: &LitStr) -> Result<(String, bool)> {
    let raw = raw_lit.value();
    let trimmed = raw.trim();

    let default_suffixes = ["(default)", "[default]"];
    for suffix in default_suffixes {
        if trimmed.len() > suffix.len() && trimmed.to_ascii_lowercase().ends_with(suffix) {
            let base = trimmed[..trimmed.len() - suffix.len()].trim();
            if base.is_empty() {
                return Err(Error::new(raw_lit.span(), "enum option id cannot be empty"));
            }
            return Ok((base.to_string(), true));
        }
    }

    if let Some(base) = trimmed.strip_suffix('*') {
        let base = base.trim();
        if base.is_empty() {
            return Err(Error::new(raw_lit.span(), "enum option id cannot be empty"));
        }
        return Ok((base.to_string(), true));
    }

    if trimmed.is_empty() {
        return Err(Error::new(raw_lit.span(), "enum option id cannot be empty"));
    }

    Ok((trimmed.to_string(), false))
}

fn enum_label_from_variant_id(variant_id: &str) -> String {
    let mut words = Vec::<String>::new();
    for chunk in variant_id.split(['_', '-', ' ']) {
        let chunk = chunk.trim();
        if chunk.is_empty() {
            continue;
        }
        let mut chars = chunk.chars();
        if let Some(first) = chars.next() {
            let mut word = String::new();
            word.push(first.to_ascii_uppercase());
            word.extend(chars);
            words.push(word);
        }
    }

    if words.is_empty() {
        variant_id.to_string()
    } else {
        words.join(" ")
    }
}

fn build_simple_enum_options_expr(spec: &SimpleEnumOptionsSpec) -> Expr {
    let entries = spec.options.iter().enumerate().map(|(index, option)| {
        let variant_id = &option.variant_id;
        let label = &option.label;
        let ordering = index as i32;
        quote! {
            golden_core::parameter::ParameterEnumOption {
                variant_id: ::std::string::String::from(#variant_id),
                value: golden_core::parameter::ParamValue::Enum(::std::string::String::from(#variant_id)),
                label: ::std::string::String::from(#label),
                tags: ::std::vec::Vec::new(),
                ordering: Some(#ordering),
            }
        }
    });

    parse_quote! {
        vec![#(#entries),*]
    }
}

fn parse_simple_string_list_expr(expr: &Expr) -> Result<Option<Vec<LitStr>>> {
    match expr {
        Expr::Array(ExprArray { elems, .. }) => {
            let mut values = Vec::new();
            for elem in elems {
                let lit = match elem {
                    Expr::Lit(ExprLit { lit: Lit::Str(lit), .. }) => lit.clone(),
                    Expr::Paren(paren) => {
                        let Expr::Lit(ExprLit { lit: Lit::Str(lit), .. }) = &*paren.expr else {
                            return Ok(None);
                        };
                        lit.clone()
                    }
                    _ => return Ok(None),
                };
                values.push(lit);
            }
            Ok(Some(values))
        }
        Expr::Paren(paren) => parse_simple_string_list_expr(&paren.expr),
        _ => Ok(None),
    }
}

fn build_file_allowed_types_assignment(expr: &Expr) -> Result<proc_macro2::TokenStream> {
    if let Some(values) = parse_simple_string_list_expr(expr)? {
        let mut parsed = Vec::<proc_macro2::TokenStream>::new();
        for value in values {
            let group = match value.value().trim().to_ascii_lowercase().as_str() {
                "audio" => quote!(golden_core::parameter::FileTypeGroup::Audio),
                "video" => quote!(golden_core::parameter::FileTypeGroup::Video),
                "script" => quote!(golden_core::parameter::FileTypeGroup::Script),
                other => {
                    return Err(Error::new(
                        value.span(),
                        format!(
                            "unsupported file type group `{other}`; expected one of: \"audio\", \"video\", \"script\""
                        ),
                    ));
                }
            };
            parsed.push(group);
        }

        return Ok(quote! {
            __param_node.constraints.file.allowed_types = vec![#(#parsed),*];
        });
    }

    Ok(quote! {
        __param_node.constraints.file.allowed_types = #expr;
    })
}

fn build_file_allowed_extensions_assignment(expr: &Expr) -> Result<proc_macro2::TokenStream> {
    if let Some(values) = parse_simple_string_list_expr(expr)? {
        return Ok(quote! {
            __param_node.constraints.file.allowed_extensions =
                vec![#(::std::string::String::from(#values)),*];
        });
    }

    Ok(quote! {
        __param_node.constraints.file.allowed_extensions = #expr;
    })
}

fn infer_enum_default_variant_from_expr(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Lit(ExprLit { lit: Lit::Str(lit), .. }) => Some(lit.value()),
        Expr::MethodCall(ExprMethodCall { receiver, method, .. }) => {
            if method == "to_string"
                && let Expr::Lit(ExprLit { lit: Lit::Str(lit), .. }) = &**receiver
            {
                return Some(lit.value());
            }
            None
        }
        Expr::Call(ExprCall { func, args, .. }) => {
            let Expr::Path(ExprPath { path, .. }) = &**func else {
                return None;
            };
            let last = path.segments.last()?.ident.to_string();
            if !(last == "new" || last == "from") {
                return None;
            }
            let arg = args.first()?;
            if let Expr::Lit(ExprLit { lit: Lit::Str(lit), .. }) = arg {
                return Some(lit.value());
            }
            None
        }
        _ => None,
    }
}

#[derive(Default)]
struct ParamsParentChildren {
    folders: Vec<usize>,
    params: Vec<usize>,
    nodes: Vec<usize>,
    ordered: Vec<ParamsChildRef>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ParamsChildRef {
    Folder(usize),
    Param(usize),
    Node(usize),
    BaseChildren,
}

struct ParamsFolderSpec {
    path: Vec<String>,
    decl_id: LitStr,
    label: LitStr,
    description: Option<LitStr>,
    reuse: bool,
    meta: ParamsDslMetaOptions,
}

enum ParamEventBehaviourSpec {
    Append,
    Coalesce,
}

enum ParamConstraintPolicySpec {
    ClampAdapt,
    Reject,
}

#[derive(Clone)]
enum ParamCallbackSpec {
    Default,
    Custom(Box<Expr>),
}

struct ParamsParamSpec {
    field: Ident,
    ty: Type,
    path: Vec<String>,
    decl_id: LitStr,
    label: LitStr,
    description: Option<LitStr>,
    meta: ParamsDslMetaOptions,
    default: Option<Expr>,
    dependency: Option<Expr>,
    behaviour: Option<ParamEventBehaviourSpec>,
    read_only: Option<Expr>,
    widget: Option<LitStr>,
    min: Option<Expr>,
    max: Option<Expr>,
    step: Option<Expr>,
    step_base: Option<Expr>,
    enum_options: Option<Expr>,
    constraint_policy: Option<ParamConstraintPolicySpec>,
    file_allowed_types: Option<Expr>,
    file_allowed_extensions: Option<Expr>,
    reference_root: Option<Expr>,
    reference_target_kind: Option<Expr>,
    reference_allowed_node_types: Option<Expr>,
    reference_allowed_parameter_types: Option<Expr>,
    reference_allow_projections: Option<Expr>,
    reference_custom_filter_key: Option<Expr>,
    reference_default_search_filter: Option<Expr>,
    callback: Option<ParamCallbackSpec>,
}

struct ParamsNodeSpec {
    field: Ident,
    ty: Type,
    path: Vec<String>,
    decl_id: LitStr,
    label: LitStr,
    description: Option<LitStr>,
    meta: ParamsDslMetaOptions,
    default: Expr,
}

#[derive(Default)]
struct ParamsPlan {
    folders: Vec<ParamsFolderSpec>,
    params: Vec<ParamsParamSpec>,
    nodes: Vec<ParamsNodeSpec>,
    children_by_parent: BTreeMap<String, ParamsParentChildren>,
    max_depth: u32,
    has_root_base_children_placeholder: bool,
    has_nested_base_children_placeholder: bool,
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
                    description: folder.description.clone(),
                    reuse: folder.reuse,
                    meta: folder.meta.clone(),
                });
                let children = plan.children_by_parent.entry(parent_key.clone()).or_default();
                children.folders.push(folder_index);
                children.ordered.push(ParamsChildRef::Folder(folder_index));

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

                let constraint_policy = if let Some(value) = param.options.policy.clone() {
                    match value.value().to_ascii_lowercase().as_str() {
                        "clampadapt" | "clamp_adapt" | "clamp-adapt" | "clamp" => {
                            Some(ParamConstraintPolicySpec::ClampAdapt)
                        }
                        "reject" => Some(ParamConstraintPolicySpec::Reject),
                        _ => {
                            return Err(Error::new(
                                value.span(),
                                "unsupported `policy`; expected \"ClampAdapt\" or \"Reject\"",
                            ));
                        }
                    }
                } else {
                    None
                };

                let callback = match (param.options.default_callback, param.options.callback.clone()) {
                    (true, Some(_)) => {
                        return Err(Error::new(
                            param.field.span(),
                            "cannot combine `default_callback` and `callback`; choose one callback style",
                        ));
                    }
                    (true, None) => Some(ParamCallbackSpec::Default),
                    (false, Some(expr)) => Some(ParamCallbackSpec::Custom(Box::new(expr))),
                    (false, None) => None,
                };

                let mut resolved_default = param.default.clone();
                let mut resolved_enum_options = param.options.enum_options.clone();
                let enum_default_override = param.options.enum_default.as_ref().map(LitStr::value);

                if let Some(enum_options_expr) = param.options.enum_options.as_ref() {
                    if let Some(simple_spec) = parse_simple_enum_options_expr(enum_options_expr)? {
                        if simple_spec.options.is_empty() {
                            return Err(Error::new(enum_options_expr.span(), "enum option list cannot be empty"));
                        }

                        let mut seen_variants = std::collections::HashSet::<String>::new();
                        let mut marked_default = None::<String>;
                        for option in &simple_spec.options {
                            let variant = option.variant_id.value();
                            if !seen_variants.insert(variant.clone()) {
                                return Err(Error::new(
                                    option.variant_id.span(),
                                    format!("duplicate enum option `{variant}`"),
                                ));
                            }
                            if option.is_default {
                                if marked_default.is_some() {
                                    return Err(Error::new(
                                        option.variant_id.span(),
                                        "multiple enum options are marked as default; only one is allowed",
                                    ));
                                }
                                marked_default = Some(variant);
                            }
                        }

                        if let Some(override_variant) = &enum_default_override
                            && !seen_variants.contains(override_variant)
                        {
                            return Err(Error::new(
                                param
                                    .options
                                    .enum_default
                                    .as_ref()
                                    .expect("enum_default present")
                                    .span(),
                                format!("`enum_default = \"{override_variant}\"` is not present in enum_options"),
                            ));
                        }

                        if let Some(default_expr) = param.default.as_ref()
                            && let Some(default_variant) = infer_enum_default_variant_from_expr(default_expr)
                            && !seen_variants.contains(&default_variant)
                        {
                            return Err(Error::new(
                                default_expr.span(),
                                format!("default enum value `{default_variant}` is not present in enum_options"),
                            ));
                        }

                        if param.default.is_some() && enum_default_override.is_some() {
                            return Err(Error::new(
                                param
                                    .options
                                    .enum_default
                                    .as_ref()
                                    .expect("enum_default present")
                                    .span(),
                                "cannot combine an explicit enum default (`= ...`) with `enum_default`; choose one",
                            ));
                        }

                        if param.default.is_none() {
                            let selected_default =
                                enum_default_override.clone().or(marked_default).unwrap_or_else(|| {
                                    simple_spec
                                        .options
                                        .first()
                                        .expect("enum options are non-empty")
                                        .variant_id
                                        .value()
                                });
                            let selected_default_lit = LitStr::new(&selected_default, param.field.span());
                            resolved_default = Some(parse_quote!(#selected_default_lit));
                        }

                        resolved_enum_options = Some(build_simple_enum_options_expr(&simple_spec));
                    } else if enum_default_override.is_some() {
                        return Err(Error::new(
                            enum_options_expr.span(),
                            "`enum_default` currently requires simple string-list enum options like `enum_options = [\"off\", \"on\", \"auto\"]`",
                        ));
                    }
                } else if enum_default_override.is_some() {
                    return Err(Error::new(
                        param
                            .options
                            .enum_default
                            .as_ref()
                            .expect("enum_default present")
                            .span(),
                        "`enum_default` requires `enum_options`",
                    ));
                }

                let param_index = plan.params.len();
                plan.params.push(ParamsParamSpec {
                    field: param.field.clone(),
                    ty: param.ty.clone(),
                    path: path.clone(),
                    decl_id: decl_id_lit,
                    label: label_lit,
                    description: param.options.description.clone(),
                    meta: param.options.meta.clone(),
                    default: resolved_default,
                    dependency: param.options.dependency.clone(),
                    behaviour,
                    read_only: param.options.read_only.clone(),
                    widget: param.options.widget.clone(),
                    min: param.options.min.clone(),
                    max: param.options.max.clone(),
                    step: param.options.step.clone(),
                    step_base: param.options.step_base.clone(),
                    enum_options: resolved_enum_options,
                    constraint_policy,
                    file_allowed_types: param.options.file_allowed_types.clone(),
                    file_allowed_extensions: param.options.file_allowed_extensions.clone(),
                    reference_root: param.options.reference_root.clone(),
                    reference_target_kind: param.options.reference_target_kind.clone(),
                    reference_allowed_node_types: param.options.reference_allowed_node_types.clone(),
                    reference_allowed_parameter_types: param.options.reference_allowed_parameter_types.clone(),
                    reference_allow_projections: param.options.reference_allow_projections.clone(),
                    reference_custom_filter_key: param.options.reference_custom_filter_key.clone(),
                    reference_default_search_filter: param.options.reference_default_search_filter.clone(),
                    callback,
                });
                let children = plan.children_by_parent.entry(parent_key.clone()).or_default();
                children.params.push(param_index);
                children.ordered.push(ParamsChildRef::Param(param_index));

                plan.max_depth = plan.max_depth.max(path.len() as u32);
            }
            ParamsDslItem::Node(node) => {
                let mut path = parent_path.to_vec();
                path.push(node.field.to_string());
                let decl_id_str = join_decl_path(&path);
                let decl_id_lit = LitStr::new(&decl_id_str, node.field.span());
                let label_lit = node
                    .options
                    .label
                    .clone()
                    .unwrap_or_else(|| LitStr::new(&node.field.to_string(), node.field.span()));

                let node_index = plan.nodes.len();
                plan.nodes.push(ParamsNodeSpec {
                    field: node.field.clone(),
                    ty: node.ty.clone(),
                    path: path.clone(),
                    decl_id: decl_id_lit,
                    label: label_lit,
                    description: node.options.description.clone(),
                    meta: node.options.meta.clone(),
                    default: node.default.clone(),
                });
                let children = plan.children_by_parent.entry(parent_key.clone()).or_default();
                children.nodes.push(node_index);
                children.ordered.push(ParamsChildRef::Node(node_index));
                plan.max_depth = plan.max_depth.max(path.len() as u32);
            }
            ParamsDslItem::BaseChildren => {
                let children = plan.children_by_parent.entry(parent_key.clone()).or_default();
                children.ordered.push(ParamsChildRef::BaseChildren);
                if parent_path.is_empty() {
                    plan.has_root_base_children_placeholder = true;
                } else {
                    plan.has_nested_base_children_placeholder = true;
                }
            }
        }
    }

    Ok(())
}

fn join_decl_path(path: &[String]) -> String {
    path.join("/")
}

fn build_declared_description_key_literal(owner_type_name: &LitStr, decl_id_lit: &LitStr) -> LitStr {
    let key = format!("{}::{}", owner_type_name.value(), decl_id_lit.value());
    LitStr::new(&key, decl_id_lit.span())
}

fn build_set_declared_description_tokens(
    target_expr: proc_macro2::TokenStream,
    owner_type_name: &LitStr,
    decl_id_lit: &LitStr,
    description_lit: &LitStr,
) -> proc_macro2::TokenStream {
    let key_lit = build_declared_description_key_literal(owner_type_name, decl_id_lit);
    quote! {
        golden_core::node::Node::node_data_mut(&mut #target_expr)
            .meta
            .set_declared_description(#key_lit, ::std::string::String::from(#description_lit));
    }
}

fn build_presentation_assignment_tokens(
    target_expr: proc_macro2::TokenStream,
    base_expr: Option<&Expr>,
    fields: &PresentationMetaFields,
) -> Option<proc_macro2::TokenStream> {
    if base_expr.is_none() && fields.is_empty() {
        return None;
    }

    let initial_value = base_expr.map_or_else(|| quote!(::std::default::Default::default()), |expr| quote!(#expr));
    let set_color = fields.color.as_ref().map(|expr| {
        quote! {
            __golden_presentation.default_color = Some((#expr).into());
        }
    });
    let set_collapsed = fields.collapsed.as_ref().map(|expr| {
        quote! {
            __golden_presentation.collapsed = #expr;
        }
    });
    let set_warnings = fields.warnings.as_ref().map(|expr| {
        quote! {
            __golden_presentation.warnings = #expr;
        }
    });
    let set_child_warning_depth = fields.show_child_warnings_max_depth.as_ref().map(|expr| {
        quote! {
            __golden_presentation.show_child_warnings_max_depth = #expr;
        }
    });
    let set_nested_inspector_visibility = fields.show_in_nested_inspector.as_ref().map(|expr| {
        quote! {
            __golden_presentation.show_in_nested_inspector = #expr;
        }
    });
    let set_inspector_content_visibility = fields.show_in_inspector_content.as_ref().map(|expr| {
        quote! {
            __golden_presentation.show_in_inspector_content = #expr;
        }
    });

    Some(quote! {
        {
            let mut __golden_presentation: golden_core::node::PresentationHint = #initial_value;
            #set_color
            #set_collapsed
            #set_warnings
            #set_child_warning_depth
            #set_nested_inspector_visibility
            #set_inspector_content_visibility
            #target_expr = __golden_presentation;
        }
    })
}

#[proc_macro_attribute]
pub fn node(attr: TokenStream, item: TokenStream) -> TokenStream {
    let node = parse_macro_input!(attr as NodeAttr);
    let input = parse_macro_input!(item as Item);
    let expansion = NodeExpansion {
        node,
        item_kind: None,
        item_menu_path: Vec::new(),
    };

    match input {
        Item::Struct(input) => expand_struct(expansion, input).into(),
        Item::Impl(input) => expand_impl(expansion, input).into(),
        other => Error::new_spanned(other, "#[node] supports only structs and `impl Node for ...` blocks")
            .to_compile_error()
            .into(),
    }
}

#[proc_macro_attribute]
pub fn item(attr: TokenStream, item: TokenStream) -> TokenStream {
    let ItemAttr {
        item_kind,
        menu_path,
        node,
    } = parse_macro_input!(attr as ItemAttr);
    let input = parse_macro_input!(item as Item);

    let resolved_item_kind = match (&input, item_kind) {
        (_, Some(item_kind)) => item_kind,
        (Item::Struct(input), None) => make_type_name_literal(&input.ident.to_string()),
        (Item::Impl(input), None) => match infer_type_name_from_impl(input) {
            Ok(item_kind) => item_kind,
            Err(err) => return err.to_compile_error().into(),
        },
        _ => unreachable!(),
    };

    let expansion = NodeExpansion {
        node,
        item_kind: Some(resolved_item_kind),
        item_menu_path: menu_path,
    };

    match input {
        Item::Struct(input) => expand_struct(expansion, input).into(),
        Item::Impl(input) => expand_impl(expansion, input).into(),
        other => Error::new_spanned(other, "#[item] supports only structs and `impl Node for ...` blocks")
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
                return Error::new_spanned(input, "#[update(...)] requires a trait impl: `impl Node for Type`")
                    .to_compile_error()
                    .into();
            };

            let is_node_impl = trait_path.segments.last().is_some_and(|seg| seg.ident == "Node");
            if !is_node_impl {
                return Error::new_spanned(trait_path, "#[update(...)] can only be used with `Node` trait impls")
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
        other => Error::new_spanned(other, "#[update(...)] supports only `impl Node for ...` blocks")
            .to_compile_error()
            .into(),
    }
}

fn expand_struct(expansion: NodeExpansion, mut input: ItemStruct) -> proc_macro2::TokenStream {
    let NodeExpansion {
        node:
            NodeAttr {
                type_name,
                ctor_meta_fields,
                presentation_fields: ctor_presentation_fields,
                via,
                impl_node,
                from_struct,
                scriptable,
                contextualizable,
            },
        item_kind,
        item_menu_path,
    } = expansion;
    if via.is_some() {
        return Error::new_spanned(input, "`via = ...` is only supported on `impl Node for ...` blocks")
            .to_compile_error();
    }
    if from_struct {
        return Error::new_spanned(input, "`from_struct` is only supported on `impl Node for ...` blocks")
            .to_compile_error();
    }
    if item_kind.is_some() && !impl_node {
        return Error::new_spanned(
            input,
            "`#[item(...)]` on a struct requires `impl_node`, or apply `#[item(...)]` on `impl Node for ...`",
        )
        .to_compile_error();
    }
    if scriptable.is_some() && !impl_node {
        return Error::new_spanned(
            input,
            "`scriptable` on a struct requires `impl_node`, or apply it on `impl Node for ...`",
        )
        .to_compile_error();
    }
    if contextualizable.is_some() && !impl_node {
        return Error::new_spanned(
            input,
            "`contextualizable` on a struct requires `impl_node`, or apply it on `impl Node for ...`",
        )
        .to_compile_error();
    }

    let generated_type_description = extract_doc_comment_literal(&input.attrs)
        .map_or_else(|| quote!(None), |description| quote!(Some(#description)));
    let mut params_dsl = None::<ParamsDsl>;
    let mut struct_defaults = BTreeMap::<String, (Ident, Expr)>::new();
    let mut kept_attrs = Vec::with_capacity(input.attrs.len());
    for attr in input.attrs.drain(..) {
        if attr
            .path()
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "children")
        {
            if params_dsl.is_some() {
                return Error::new_spanned(attr, "only one #[children(...)] attribute is supported per struct")
                    .to_compile_error();
            }
            let parsed = match attr.parse_args::<ParamsDsl>() {
                Ok(parsed) => parsed,
                Err(err) => return err.to_compile_error(),
            };
            params_dsl = Some(parsed);
        } else if attr
            .path()
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "defaults")
        {
            if !struct_defaults.is_empty() {
                return Error::new_spanned(attr, "only one #[defaults(...)] attribute is supported per struct")
                    .to_compile_error();
            }
            let parsed = match attr.parse_args::<StructDefaultsAttr>() {
                Ok(parsed) => parsed,
                Err(err) => return err.to_compile_error(),
            };
            struct_defaults = parsed.values;
        } else {
            kept_attrs.push(attr);
        }
    }
    input.attrs = kept_attrs;

    let struct_name = input.ident.clone();
    let resolved_type_name = type_name.unwrap_or_else(|| make_type_name_literal(&struct_name.to_string()));
    let fallback_default_label = make_label_literal(&resolved_type_name.value());
    let static_default_label = ctor_meta_fields
        .get("label")
        .and_then(|(_, expr)| expr_string_literal(expr))
        .unwrap_or_else(|| fallback_default_label.clone());
    let generics = input.generics.clone();
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let params_plan = if let Some(params_dsl) = &params_dsl {
        match build_params_plan(params_dsl) {
            Ok(plan) => Some(plan),
            Err(err) => return err.to_compile_error(),
        }
    } else {
        None
    };

    let fields = match &mut input.fields {
        Fields::Named(named) => &mut named.named,
        _ => {
            return Error::new_spanned(input, "#[node(\"...\")] supports only structs with named fields")
                .to_compile_error();
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
    let mut param_runtime_sync_bindings = Vec::<proc_macro2::TokenStream>::new();
    let mut param_change_callback_statements = Vec::<proc_macro2::TokenStream>::new();
    let mut param_refresh_bindings = Vec::<proc_macro2::TokenStream>::new();
    let mut param_dependency_reconcile_statements = Vec::<proc_macro2::TokenStream>::new();
    let mut param_order_reconcile_statements = Vec::<proc_macro2::TokenStream>::new();
    let mut persisted_state_fields = Vec::<(Ident, LitStr, Option<Expr>)>::new();
    let field_param_order = fields
        .iter()
        .filter_map(|field| {
            let ident = field.ident.clone()?;
            field
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident("param"))
                .then_some(ident)
        })
        .collect::<Vec<_>>();
    let mut field_param_prev_decl_ids = Vec::<LitStr>::new();

    for field in fields.iter_mut() {
        let Some(field_ident) = field.ident.clone() else {
            continue;
        };

        if field_ident == "node_data" {
            continue;
        }

        let field_default = struct_defaults.remove(&field_ident.to_string()).map(|(_, expr)| expr);
        let (param_attr, potential_attr, state_attr) = match take_handle_attrs(field) {
            Ok(attrs) => attrs,
            Err(err) => return err.to_compile_error(),
        };
        if param_attr.is_some() && potential_attr.is_some() {
            return Error::new_spanned(field, "field cannot have both #[param(...)] and #[potential_node(...)]")
                .to_compile_error();
        }
        if state_attr.is_some() && (param_attr.is_some() || potential_attr.is_some()) {
            return Error::new_spanned(field, "#[state(...)] is only supported on plain node fields")
                .to_compile_error();
        }

        let state_args = if let Some(state_attr) = state_attr {
            let args = match state_attr.parse_args::<StateFieldArgs>() {
                Ok(args) => args,
                Err(err) => return err.to_compile_error(),
            };
            Some(args)
        } else {
            None
        };

        if let Some(param_attr) = param_attr {
            if field_default.is_some() {
                return Error::new_spanned(
                    field,
                    "cannot combine #[defaults(...)] with #[param(...)] field declarations",
                )
                .to_compile_error();
            }
            if params_plan.is_some() {
                return Error::new_spanned(param_attr, "cannot combine field-level #[param(...)] with struct-level #[children(...)]; choose one parameter declaration style").to_compile_error();
            }
            let args = match param_attr.parse_args::<ParamFieldArgs>() {
                Ok(args) => args,
                Err(err) => return err.to_compile_error(),
            };

            let Some(default_expr) = args.default else {
                return Error::new_spanned(param_attr, "#[param(...)] requires `default = ...`").to_compile_error();
            };

            let Some(param_value_ty) = extract_handle_inner_type(&field.ty, "ParameterHandle") else {
                return Error::new_spanned(&field.ty, "#[param(...)] requires field type ParameterHandle<T>")
                    .to_compile_error();
            };

            let decl_id_lit = args
                .decl_id
                .unwrap_or_else(|| LitStr::new(&field_ident.to_string(), field_ident.span()));
            let label_lit = args
                .label
                .unwrap_or_else(|| LitStr::new(&field_ident.to_string(), field_ident.span()));
            let dependency_predicate = match args.dependency.as_ref() {
                Some(expr) => match build_param_dependency_eval_tokens(expr, &field_param_order) {
                    Ok(tokens) => Some(tokens),
                    Err(err) => return err.to_compile_error(),
                },
                None => None,
            };
            let previous_decl_ids = field_param_prev_decl_ids.clone();
            field_param_prev_decl_ids.push(decl_id_lit.clone());
            let insert_after = build_declared_prev_sibling_tokens(&previous_decl_ids, quote!(__golden_node_owner_id));
            let set_description = args.description.as_ref().map(|description_lit| {
                build_set_declared_description_tokens(
                    quote!(__param_node),
                    &resolved_type_name,
                    &decl_id_lit,
                    description_lit,
                )
            });
            let set_range = build_range_constraint_assignment(args.min.as_ref(), args.max.as_ref(), &param_value_ty);
            let set_read_only = args.read_only.map(|expr| {
                quote! {
                    __param_node.read_only = #expr;
                }
            });
            let set_widget = args.widget.as_ref().map(|widget| {
                quote! {
                    __param_node.ui_hints.widget = Some(::std::string::String::from(#widget));
                }
            });
            let sync_bound_widget = args.widget.as_ref().map(|widget| {
                quote! {
                    golden_core::node::NodeHandle::new(self.#field_ident.id()).with_mut_leaf::<golden_core::parameter::Parameter, _>(
                        ctx,
                        |__param_node, _child_ctx| {
                            if __param_node.ui_hints.widget.as_deref() != Some(#widget) {
                                __param_node.ui_hints.widget = Some(::std::string::String::from(#widget));
                            }
                        },
                    );
                }
            });
            let set_step = args.step.map(|expr| {
                quote! {
                    __param_node.constraints.step = Some((#expr) as f64);
                }
            });
            let set_step_base = args.step_base.map(|expr| {
                quote! {
                    __param_node.constraints.step_base = Some((#expr) as f64);
                }
            });
            let set_enum_options = args.enum_options.map(|expr| {
                quote! {
                    __param_node.constraints.enum_options = #expr;
                }
            });
            let set_file_allowed_types = match args.file_allowed_types.as_ref() {
                Some(expr) => match build_file_allowed_types_assignment(expr) {
                    Ok(tokens) => Some(tokens),
                    Err(err) => return err.to_compile_error(),
                },
                None => None,
            };
            let set_file_allowed_extensions = match args.file_allowed_extensions.as_ref() {
                Some(expr) => match build_file_allowed_extensions_assignment(expr) {
                    Ok(tokens) => Some(tokens),
                    Err(err) => return err.to_compile_error(),
                },
                None => None,
            };
            let set_constraint_policy = if let Some(value) = args.policy {
                match value.value().to_ascii_lowercase().as_str() {
                    "clampadapt" | "clamp_adapt" | "clamp-adapt" | "clamp" => Some(quote! {
                        __param_node.constraints.policy = golden_core::parameter::ParameterConstraintPolicy::ClampAdapt;
                    }),
                    "reject" => Some(quote! {
                        __param_node.constraints.policy = golden_core::parameter::ParameterConstraintPolicy::Reject;
                    }),
                    _ => {
                        return Error::new(
                            value.span(),
                            "unsupported #[param(...)] `policy`; expected \"ClampAdapt\" or \"Reject\"",
                        )
                        .to_compile_error();
                    }
                }
            } else {
                None
            };
            let callback = if args.default_callback {
                Some(ParamCallbackSpec::Default)
            } else {
                args.callback
                    .map(|callback| ParamCallbackSpec::Custom(Box::new(callback)))
            };
            let create_param_node = quote! {
                {
                    let mut __param_node = golden_core::parameter::Parameter::new(
                        #label_lit,
                        <#param_value_ty as golden_core::node::ParameterValueType>::to_param_value(
                            self.#field_ident.get_ref().clone()
                        ),
                        self.#field_ident.change_check().clone(),
                    );
                    __param_node.event_behaviour = self.#field_ident.event_behaviour();
                    #set_read_only
                    #set_widget
                    #set_range
                    #set_step
                    #set_step_base
                    #set_enum_options
                    #set_file_allowed_types
                    #set_file_allowed_extensions
                    #set_constraint_policy
                    golden_core::node::Node::node_data_mut(&mut __param_node).meta.decl_id =
                        golden_core::node::DeclId(::std::string::String::from(#decl_id_lit));
                    #set_description
                    ctx.add_child(__golden_node_owner_id, __param_node, #insert_after);
                }
            };
            let bind_existing_param_node = quote! {
                if let Some(__golden_snapshot) = ctx.tree_snapshot() {
                    if let Some(__golden_existing_node_id) =
                        __golden_snapshot.find_child_by_decl_id(__golden_node_owner_id, #decl_id_lit)
                    {
                        self.#field_ident.set_node_id(__golden_existing_node_id);
                        if let Some(__golden_existing_value) = __golden_snapshot
                            .node(__golden_existing_node_id)
                            .and_then(|__golden_existing_node| __golden_existing_node.param_value.as_ref())
                        {
                            let _ = self.#field_ident.apply_runtime_value(__golden_existing_value);
                        }
                    } else if self.#field_ident.is_bound()
                        && __golden_snapshot.node(self.#field_ident.id()).is_none()
                    {
                        self.#field_ident.clear_node_id();
                    }
                }
            };

            ctor_inits.push(quote! {
                #field_ident: golden_core::node::ParameterHandle::<#param_value_ty>::new(#default_expr)
            });

            if let Some(predicate) = &dependency_predicate {
                generated_init_statements.push(quote! {
                    #bind_existing_param_node
                    if #predicate && !self.#field_ident.is_bound() {
                        #create_param_node
                    }
                });
                param_dependency_reconcile_statements.push(quote! {
                    if #predicate {
                        if !self.#field_ident.is_bound() {
                            #create_param_node
                        }
                    } else if self.#field_ident.is_bound() {
                        ctx.edits.push(golden_core::edit::Edit::RemoveNode { node: self.#field_ident.id() });
                        // Clear the binding immediately rather than waiting for the
                        // `ChildRemoved` event to round-trip: this reconcile can run more
                        // than once against the same shared tree snapshot within a single
                        // dispatch batch (e.g. once via the parent's own on_inbox callback
                        // and once via this node's own preprocess pass), and without this
                        // the second pass still sees `is_bound() == true` and queues a
                        // second `RemoveNode` for an id the engine already removed.
                        self.#field_ident.clear_node_id();
                    }
                });
                param_order_reconcile_statements.push(quote! {
                    if #predicate && self.#field_ident.is_bound() {
                        let __golden_node_id = self.#field_ident.id();
                        if __golden_node_id.0 != 0 {
                            if let Some(__golden_snapshot) = ctx.tree_snapshot() {
                                let __golden_expected_prev = #insert_after;
                                let __golden_current_prev = __golden_snapshot.previous_sibling(__golden_node_owner_id, __golden_node_id);
                                if __golden_current_prev != __golden_expected_prev {
                                    ctx.edits.push(golden_core::edit::Edit::MoveNode {
                                        node: __golden_node_id,
                                        new_parent: __golden_node_owner_id,
                                        new_prev_sibling: __golden_expected_prev,
                                    });
                                }
                            }
                        }
                    }
                });
            } else {
                generated_init_statements.push(quote! {
                    #bind_existing_param_node
                    if !self.#field_ident.is_bound() {
                        #create_param_node
                    }
                });
            }

            child_added_decl_statements.push(quote! {
                if parent == __golden_node_owner_id && decl_id.0 == #decl_id_lit {
                    self.#field_ident.set_node_id(child);
                    #sync_bound_widget
                }
            });

            child_replaced_decl_statements.push(quote! {
                if parent == __golden_node_owner_id && decl_id.0 == #decl_id_lit {
                    self.#field_ident.set_node_id(new);
                    #sync_bound_widget
                }
            });

            child_removed_statements.push(quote! {
                if parent == __golden_node_owner_id && self.#field_ident.id() == child {
                    self.#field_ident.clear_node_id();
                }
            });

            param_runtime_sync_bindings.push(quote! {
                if self.#field_ident.id() == param {
                    let _ = self.#field_ident.apply_runtime_value(new_value);
                }
            });

            if let Some(callback_spec) = &callback {
                param_change_callback_statements
                    .push(build_param_callback_dispatch(field_ident.clone(), callback_spec));
            }

            param_refresh_bindings.push(quote! {
                if self.#field_ident.is_bound() {
                    if let Some(value) = resolve(self.#field_ident.id()) {
                        let _ = self.#field_ident.apply_runtime_value(&value);
                    }
                }
            });

            continue;
        }

        if let Some(potential_attr) = potential_attr {
            if field_default.is_some() {
                return Error::new_spanned(
                    field,
                    "cannot combine #[defaults(...)] with #[potential_node(...)] field declarations",
                )
                .to_compile_error();
            }
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

            let decl_id_lit = args
                .decl_id
                .unwrap_or_else(|| LitStr::new(&field_ident.to_string(), field_ident.span()));

            ctor_inits.push(quote! {
                #field_ident: golden_core::node::PotentialNodeHandle::new(
                    golden_core::node::NodeId(0),
                    #decl_id_lit
                )
            });

            generated_init_statements.push(quote! {
                self.#field_ident.set_parent(__golden_node_owner_id);
            });

            child_added_decl_statements.push(quote! {
                let _ = self.#field_ident.reconcile_child_added(parent, child, &decl_id);
            });

            child_replaced_decl_statements.push(quote! {
                let _ = self.#field_ident.reconcile_child_replaced(parent, old, new, &decl_id);
            });

            child_removed_statements.push(quote! {
                let _ = self.#field_ident.reconcile_child_removed(parent, child);
            });

            continue;
        }

        let state_default = state_args.as_ref().and_then(|args| args.default.clone());
        if field_default.is_some() && state_default.is_some() {
            return Error::new_spanned(
                field,
                "cannot define both #[defaults(...)] and #[state(default = ...)] for the same field",
            )
            .to_compile_error();
        }

        if state_args.as_ref().and_then(|args| args.persist).unwrap_or(false) {
            persisted_state_fields.push((
                field_ident.clone(),
                LitStr::new(&field_ident.to_string(), field_ident.span()),
                state_default.clone().or(field_default.clone()),
            ));
        }

        if let Some(default_expr) = state_default.or(field_default) {
            ctor_inits.push(quote! { #field_ident: #default_expr });
        } else {
            ctor_fields.push((field_ident.clone(), field.ty.clone()));
            ctor_inits.push(quote! { #field_ident });
        }
    }

    if let Some((_, (field, _))) = struct_defaults.iter().next() {
        return Error::new(field.span(), format!("unknown #[defaults(...)] field `{}`", field)).to_compile_error();
    }

    let mut generated_child_interest_depth = if child_added_decl_statements.is_empty()
        && child_replaced_decl_statements.is_empty()
        && child_removed_statements.is_empty()
    {
        0u32
    } else {
        1u32
    };

    if let Some(plan) = &params_plan {
        let plan_param_fields = plan.params.iter().map(|param| param.field.clone()).collect::<Vec<_>>();
        for param in &plan.params {
            if fields
                .iter()
                .any(|field| field.ident.as_ref().is_some_and(|ident| ident == &param.field))
            {
                return Error::new(
                    param.field.span(),
                    format!("duplicate field `{}` generated by #[children(...)]", param.field),
                )
                .to_compile_error();
            }
            let field_ident = &param.field;
            let ty = &param.ty;
            fields.push(parse_quote! {
                #field_ident: golden_core::node::ParameterHandle<#ty>
            });
            if let Some(default_expr) = &param.default {
                ctor_inits.push(quote! {
                    #field_ident: golden_core::node::ParameterHandle::<#ty>::new((#default_expr).into())
                });
            } else {
                ctor_inits.push(quote! {
                    #field_ident: golden_core::node::ParameterHandle::<#ty>::unbound()
                });
            }
        }

        for node in &plan.nodes {
            if fields
                .iter()
                .any(|field| field.ident.as_ref().is_some_and(|ident| ident == &node.field))
            {
                return Error::new(
                    node.field.span(),
                    format!("duplicate field `{}` generated by #[children(...)]", node.field),
                )
                .to_compile_error();
            }

            let field_ident = &node.field;
            let ty = &node.ty;
            let decl_id_lit = &node.decl_id;
            fields.push(parse_quote! {
                #field_ident: golden_core::node::DeclaredNodeHandle<#ty>
            });
            ctor_inits.push(quote! {
                #field_ident: golden_core::node::DeclaredNodeHandle::<#ty>::new(
                    golden_core::node::NodeId(0),
                    #decl_id_lit
                )
            });
        }

        let root_materialize =
            materialize_children_tokens(plan, &resolved_type_name, "", quote!(__golden_node_owner_id));
        generated_init_statements.extend(root_materialize);

        for folder in &plan.folders {
            let decl_id_lit = &folder.decl_id;
            let folder_key = join_decl_path(&folder.path);
            let materialize = materialize_children_tokens(plan, &resolved_type_name, &folder_key, quote!(child));
            child_added_decl_statements.push(quote! {
                if decl_id.0 == #decl_id_lit {
                    #(#materialize)*
                }
            });
        }

        for param in &plan.params {
            let decl_id_lit = &param.decl_id;
            let field_ident = &param.field;
            let sync_bound_widget = param.widget.as_ref().map(|widget| {
                quote! {
                    golden_core::node::NodeHandle::new(self.#field_ident.id()).with_mut_leaf::<golden_core::parameter::Parameter, _>(
                        ctx,
                        |__param_node, _child_ctx| {
                            if __param_node.ui_hints.widget.as_deref() != Some(#widget) {
                                __param_node.ui_hints.widget = Some(::std::string::String::from(#widget));
                            }
                        },
                    );
                }
            });
            child_added_decl_statements.push(quote! {
                if decl_id.0 == #decl_id_lit {
                    self.#field_ident.set_node_id(child);
                    #sync_bound_widget
                }
            });
        }

        for node in &plan.nodes {
            let field_ident = &node.field;
            child_added_decl_statements.push(quote! {
                let _ = self.#field_ident.reconcile_child_added(parent, child, &decl_id);
            });
        }

        for folder in &plan.folders {
            let decl_id_lit = &folder.decl_id;
            let folder_key = join_decl_path(&folder.path);
            let materialize = materialize_children_tokens(plan, &resolved_type_name, &folder_key, quote!(new));
            child_replaced_decl_statements.push(quote! {
                if decl_id.0 == #decl_id_lit {
                    #(#materialize)*
                }
            });
        }

        for param in &plan.params {
            let decl_id_lit = &param.decl_id;
            let field_ident = &param.field;
            let sync_bound_widget = param.widget.as_ref().map(|widget| {
                quote! {
                    golden_core::node::NodeHandle::new(self.#field_ident.id()).with_mut_leaf::<golden_core::parameter::Parameter, _>(
                        ctx,
                        |__param_node, _child_ctx| {
                            if __param_node.ui_hints.widget.as_deref() != Some(#widget) {
                                __param_node.ui_hints.widget = Some(::std::string::String::from(#widget));
                            }
                        },
                    );
                }
            });
            child_replaced_decl_statements.push(quote! {
                if decl_id.0 == #decl_id_lit {
                    self.#field_ident.set_node_id(new);
                    #sync_bound_widget
                }
            });
        }

        for node in &plan.nodes {
            let field_ident = &node.field;
            child_replaced_decl_statements.push(quote! {
                let _ = self.#field_ident.reconcile_child_replaced(parent, old, new, &decl_id);
            });
        }

        for param in &plan.params {
            let field_ident = &param.field;
            child_removed_statements.push(quote! {
                if self.#field_ident.id() == child {
                    self.#field_ident.clear_node_id();
                }
            });
        }

        for node in &plan.nodes {
            let field_ident = &node.field;
            generated_init_statements.push(quote! {
                self.#field_ident.set_parent(__golden_node_owner_id);
            });
            child_removed_statements.push(quote! {
                let _ = self.#field_ident.reconcile_child_removed(parent, child);
            });
        }

        for param in &plan.params {
            let field_ident = &param.field;
            param_runtime_sync_bindings.push(quote! {
                if self.#field_ident.id() == param {
                    let _ = self.#field_ident.apply_runtime_value(new_value);
                }
            });
        }

        for param in &plan.params {
            let field_ident = param.field.clone();
            if let Some(callback_spec) = &param.callback {
                param_change_callback_statements.push(build_param_callback_dispatch(field_ident, callback_spec));
            }
        }

        for param in &plan.params {
            let field_ident = &param.field;
            param_refresh_bindings.push(quote! {
                if self.#field_ident.is_bound() {
                    if let Some(value) = resolve(self.#field_ident.id()) {
                        let _ = self.#field_ident.apply_runtime_value(&value);
                    }
                }
            });
        }

        for (param_index, param) in plan.params.iter().enumerate() {
            let Some(dependency_expr) = param.dependency.as_ref() else {
                continue;
            };
            let dependency_predicate = match build_param_dependency_eval_tokens(dependency_expr, &plan_param_fields) {
                Ok(tokens) => tokens,
                Err(err) => return err.to_compile_error(),
            };
            let field_ident = &param.field;
            let parent_path = join_decl_path(&param.path[..param.path.len().saturating_sub(1)]);
            let parent_path_lit = LitStr::new(&parent_path, proc_macro2::Span::call_site());
            let expected_prev = match build_plan_prev_sibling_tokens(
                plan,
                &parent_path,
                ParamsChildRef::Param(param_index),
                quote!(__golden_parent),
                &plan_param_fields,
            ) {
                Ok(tokens) => tokens,
                Err(err) => return err.to_compile_error(),
            };
            let create_param = build_params_plan_param_create_tokens_with_insert_after(
                plan,
                &resolved_type_name,
                quote!(__golden_parent),
                field_ident,
                expected_prev.clone(),
            );
            let resolve_parent = if parent_path.is_empty() {
                quote!(Some(__golden_node_owner_id))
            } else {
                quote!(ctx.tree_snapshot().and_then(|snapshot| snapshot.resolve_path_from(__golden_node_owner_id, #parent_path_lit)))
            };
            param_dependency_reconcile_statements.push(quote! {
                {
                    let __golden_parent = #resolve_parent;
                    if let Some(__golden_parent) = __golden_parent {
                        if #dependency_predicate {
                            if !self.#field_ident.is_bound() {
                                #create_param
                            }
                        } else if self.#field_ident.is_bound() {
                            ctx.edits.push(golden_core::edit::Edit::RemoveNode { node: self.#field_ident.id() });
                            // See the matching comment on the sibling reconcile arm above:
                            // clear eagerly so a second reconcile pass in the same dispatch
                            // batch doesn't queue a duplicate `RemoveNode` for this id.
                            self.#field_ident.clear_node_id();
                        }
                    }
                }
            });
            param_order_reconcile_statements.push(quote! {
                if let Some(__golden_parent) = #resolve_parent {
                    if #dependency_predicate && self.#field_ident.is_bound() {
                        let __golden_node_id = self.#field_ident.id();
                        if __golden_node_id.0 != 0 {
                            if let Some(__golden_snapshot) = ctx.tree_snapshot() {
                                let __golden_expected_prev = #expected_prev;
                                let __golden_current_prev = __golden_snapshot.previous_sibling(__golden_parent, __golden_node_id);
                                if __golden_current_prev != __golden_expected_prev {
                                    ctx.edits.push(golden_core::edit::Edit::MoveNode {
                                        node: __golden_node_id,
                                        new_parent: __golden_parent,
                                        new_prev_sibling: __golden_expected_prev,
                                    });
                                }
                            }
                        }
                    }
                }
            });
        }

        generated_child_interest_depth = generated_child_interest_depth.max(plan.max_depth.max(1));
    }

    let generated_has_root_base_children_placeholder = params_plan
        .as_ref()
        .is_some_and(|plan| plan.has_root_base_children_placeholder);
    let generated_has_nested_base_children_placeholder = params_plan
        .as_ref()
        .is_some_and(|plan| plan.has_nested_base_children_placeholder);

    let ctor_args = ctor_fields
        .iter()
        .map(|(ident, ty)| quote!(#ident: #ty))
        .collect::<Vec<_>>();
    // Plain node fields define the public constructor contract. Replacing a high-arity `new`
    // with a generated config type would break that contract while adding a public type solely
    // to satisfy a style lint, so exempt only constructors whose generated arity exceeds it.
    let generated_constructor_arity_exemption = (ctor_fields.len() > 7).then(|| {
        quote! {
            #[allow(
                clippy::too_many_arguments,
                reason = "the node constructor mirrors its macro-declared fields and is generated code"
            )]
        }
    });
    let generated_default_label = match ctor_meta_fields.get("label") {
        Some((_, expr)) => quote! { (#expr).into() },
        None => quote! { ::std::string::String::from(Self::DEFAULT_LABEL) },
    };
    let mut ctor_meta_fields = ctor_meta_fields;
    let ctor_presentation_expr = ctor_meta_fields.remove("presentation").map(|(_, expr)| expr);
    let ctor_meta_inits = ctor_meta_fields
        .values()
        .map(|(field_ident, expr)| {
            let field_name = field_ident.to_string();
            if field_name == "label" || field_name == "short_name" {
                quote! {
                    node_data.meta.#field_ident = (#expr).into();
                }
            } else {
                quote! {
                    node_data.meta.#field_ident = #expr;
                }
            }
        })
        .collect::<Vec<_>>();
    let ctor_presentation_init = build_presentation_assignment_tokens(
        quote!(node_data.meta.presentation),
        ctor_presentation_expr.as_ref(),
        &ctor_presentation_fields,
    );
    let generated_project_create = if ctor_fields.is_empty() {
        quote! {
            if node_type == #resolved_type_name {
                Some(Self::new())
            } else {
                None
            }
        }
    } else {
        quote! {
            let _ = node_type;
            None
        }
    };
    let generated_project_encode_data = if persisted_state_fields.is_empty() {
        quote! {
            Ok(serde_json::Value::Null)
        }
    } else {
        let inserts = persisted_state_fields
            .iter()
            .map(|(field_ident, field_name, default_expr)| {
                if let Some(default_expr) = default_expr {
                    quote! {
                        let __golden_current_value = serde_json::to_value(&self.#field_ident)
                            .map_err(|err| format!("failed to encode '{}' field: {err}", #field_name))?;
                        let __golden_default_value = serde_json::to_value(&(#default_expr))
                            .map_err(|err| format!("failed to encode default for '{}' field: {err}", #field_name))?;
                        if __golden_current_value != __golden_default_value {
                            __golden_data.insert(
                                ::std::string::ToString::to_string(#field_name),
                                __golden_current_value,
                            );
                        }
                    }
                } else {
                    quote! {
                        __golden_data.insert(
                            ::std::string::ToString::to_string(#field_name),
                            serde_json::to_value(&self.#field_ident)
                                .map_err(|err| format!("failed to encode '{}' field: {err}", #field_name))?,
                        );
                    }
                }
            });
        quote! {
            let mut __golden_data = serde_json::Map::new();
            #(#inserts)*
            if __golden_data.is_empty() {
                Ok(serde_json::Value::Null)
            } else {
                Ok(serde_json::Value::Object(__golden_data))
            }
        }
    };
    let generated_project_decode_data = if persisted_state_fields.is_empty() {
        quote! {
            if data.is_null() {
                return Ok(());
            }

            Err(format!("node type '{}' does not support persisted project data", #resolved_type_name))
        }
    } else {
        let decodes = persisted_state_fields
            .iter()
            .map(|(field_ident, field_name, _default_expr)| {
                quote! {
                    if let Some(__golden_value) = __golden_object.get(#field_name) {
                        self.#field_ident = serde_json::from_value(__golden_value.clone())
                            .map_err(|err| format!("invalid '{}' field: {err}", #field_name))?;
                    }
                }
            });
        quote! {
            if data.is_null() {
                return Ok(());
            }

            let Some(__golden_object) = data.as_object() else {
                return Err(format!("node type '{}' expects persisted project data as an object", #resolved_type_name));
            };

            #(#decodes)*
            Ok(())
        }
    };

    let generated_user_item_kind = item_kind.as_ref().map(|item_kind| {
        quote! {
            fn user_item_kind(&self) -> &str {
                #item_kind
            }
        }
    });
    let generated_declared_user_item = item_kind.as_ref().map(|_| {
        quote! {
            fn is_declared_user_item(&self) -> bool {
                true
            }
        }
    });
    let generated_create_alias = ctor_fields.is_empty().then(|| {
        quote! {
            /// Creates a new node instance using the declared default item construction path.
            pub fn create() -> Self {
                Self::new()
            }
        }
    });
    let generated_item_menu_path = build_item_menu_path_tokens(&item_menu_path);
    let generated_declared_user_item_node = item_kind.as_ref().map(|item_kind| {
        quote! {
            impl #impl_generics golden_core::node::DeclaredUserItemNode for #struct_name #ty_generics #where_clause {
                const ITEM_NODE_TYPE: &'static str = #resolved_type_name;
                const ITEM_KIND: &'static str = #item_kind;

                fn item_default_label() -> ::std::string::String {
                    Self::default_label()
                }

                fn item_menu_path() -> ::std::vec::Vec<::std::string::String> {
                    #generated_item_menu_path
                }

                fn create_item() -> Self {
                    Self::create()
                }
            }
        }
    });
    let generated_script_host_policy = scriptable.as_ref().map(build_script_host_policy_method_tokens);
    let generated_user_context_host_policy = contextualizable
        .as_ref()
        .map(build_user_context_host_policy_method_tokens);

    let generated_node_impl = if impl_node {
        quote! {
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

                fn type_description(&self) -> Option<&str> {
                    Self::__golden_node_type_description()
                }

                fn as_any(&self) -> &dyn std::any::Any {
                    self
                }

                fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                    self
                }

                #generated_user_item_kind
                #generated_declared_user_item
                #generated_script_host_policy
                #generated_user_context_host_policy

                fn engine_child_event_interest_depth(&self, event: &golden_core::events::Event) -> u32 {
                    self.__golden_node_engine_child_event_interest_depth(event)
                }

                fn engine_on_attached(&mut self, ctx: &mut golden_core::process_ctx::ProcessCtx) {
                    self.__golden_node_engine_on_attached(ctx, self.node_data.id);
                }

                fn engine_sync_param_handle_cache(
                    &mut self,
                    param: golden_core::node::NodeId,
                    new_value: &golden_core::parameter::ParamValue,
                ) {
                    self.__golden_node_engine_sync_param_handle_cache(param, new_value);
                }

                fn engine_sync_bound_param_handles(
                    &mut self,
                    resolve: &mut dyn FnMut(golden_core::node::NodeId) -> Option<golden_core::parameter::ParamValue>,
                ) {
                    self.__golden_node_engine_sync_bound_param_handles(resolve);
                }

                fn engine_materialize_declared_inbox(
                    &mut self,
                    ctx: &mut golden_core::process_ctx::ProcessCtx,
                ) {
                    self.__golden_node_engine_materialize_declared_inbox(ctx, self.node_data.id);
                }

                fn engine_preprocess_inbox(&mut self, ctx: &mut golden_core::process_ctx::ProcessCtx) {
                    self.__golden_node_engine_preprocess_inbox(ctx, self.node_data.id);
                }
            }
        }
    } else {
        quote! {}
    };

    quote! {
        #input

        impl #impl_generics #struct_name #ty_generics #where_clause {
            /// Runtime node type identifier declared by the node macro.
            pub const NODE_TYPE: &'static str = #resolved_type_name;

            /// Static fallback label exposed for catalog and schema code.
            pub const DEFAULT_LABEL: &'static str = #static_default_label;

            #[doc(hidden)]
            pub const __GOLDEN_NODE_HAS_ROOT_BASE_CHILDREN_PLACEHOLDER: bool =
                #generated_has_root_base_children_placeholder;

            #[doc(hidden)]
            pub const __GOLDEN_NODE_HAS_NESTED_BASE_CHILDREN_PLACEHOLDER: bool =
                #generated_has_nested_base_children_placeholder;

            /// Returns the runtime default label used by the generated constructor.
            pub fn default_label() -> ::std::string::String {
                #generated_default_label
            }

            /// Creates a new node instance with its declared default label, handles, and state.
            #generated_constructor_arity_exemption
            pub fn new(#(#ctor_args),*) -> Self {
                let mut node_data = golden_core::node::NodeData::new(Self::default_label());
                #(#ctor_meta_inits)*
                #ctor_presentation_init
                Self {
                    node_data,
                    #(#ctor_inits),*
                }
            }

            #generated_create_alias

            #[doc(hidden)]
            pub fn __golden_node_type_description() -> Option<&'static str> {
                #generated_type_description
            }

            #[doc(hidden)]
            pub fn __golden_node_project_create(node_type: &str) -> Option<Self> {
                #generated_project_create
            }

            #[doc(hidden)]
            pub fn __golden_node_project_encode_data(&self) -> Result<serde_json::Value, String> {
                #generated_project_encode_data
            }

            #[doc(hidden)]
            pub fn __golden_node_project_decode_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
                #generated_project_decode_data
            }

            #[doc(hidden)]
            pub fn __golden_node_engine_child_event_interest_depth(&self, _event: &golden_core::events::Event) -> u32 {
                #generated_child_interest_depth
            }

            #[doc(hidden)]
            pub fn __golden_node_engine_on_attached(
                &mut self,
                ctx: &mut golden_core::process_ctx::ProcessCtx,
                owner_id: golden_core::node::NodeId,
            ) {
                self.__golden_node_engine_on_attached_with_base_children(ctx, owner_id, |_, _| {});
            }

            #[doc(hidden)]
            pub fn __golden_node_engine_on_attached_with_base_children<F>(
                &mut self,
                ctx: &mut golden_core::process_ctx::ProcessCtx,
                owner_id: golden_core::node::NodeId,
                mut __golden_base_children: F,
            )
            where
                F: FnMut(&mut Self, &mut golden_core::process_ctx::ProcessCtx),
            {
                let __golden_node_owner_id = owner_id;
                let mut __golden_base_children_inserted = false;
                let _ = &mut __golden_base_children;
                let _ = &mut __golden_base_children_inserted;
                #(#generated_init_statements)*
            }

            #[doc(hidden)]
            pub fn __golden_node_engine_sync_param_handle_cache(
                &mut self,
                param: golden_core::node::NodeId,
                new_value: &golden_core::parameter::ParamValue,
            ) {
                #(#param_runtime_sync_bindings)*
            }

            #[doc(hidden)]
            pub fn __golden_node_engine_sync_bound_param_handles(
                &mut self,
                resolve: &mut dyn FnMut(golden_core::node::NodeId) -> Option<golden_core::parameter::ParamValue>,
            ) {
                #(#param_refresh_bindings)*
            }

            #[doc(hidden)]
            pub fn __golden_node_engine_materialize_declared_inbox(
                &mut self,
                ctx: &mut golden_core::process_ctx::ProcessCtx,
                owner_id: golden_core::node::NodeId,
            ) {
                self.__golden_node_engine_materialize_declared_inbox_with_base_children(
                    ctx,
                    owner_id,
                    |_, _| {},
                );
            }

            #[doc(hidden)]
            pub fn __golden_node_engine_materialize_declared_inbox_with_base_children<F>(
                &mut self,
                ctx: &mut golden_core::process_ctx::ProcessCtx,
                owner_id: golden_core::node::NodeId,
                mut __golden_base_children: F,
            )
            where
                F: FnMut(&mut Self, &mut golden_core::process_ctx::ProcessCtx),
            {
                let __golden_node_owner_id = owner_id;
                let mut __golden_base_children_inserted = false;
                let __golden_events = ctx.events.clone();
                for event in &__golden_events {
                    match &event.kind {
                        golden_core::events::EventKind::ChildAdded { parent, child, decl_id } => {
                            let parent = *parent;
                            let child = *child;
                            let decl_id = decl_id.clone();
                            #(#child_added_decl_statements)*
                        }
                        golden_core::events::EventKind::ChildReplaced { parent, old, new, decl_id } => {
                            let parent = *parent;
                            let old = *old;
                            let new = *new;
                            let decl_id = decl_id.clone();
                            #(#child_replaced_decl_statements)*
                        }
                        _ => {}
                    }
                }
                if !__golden_base_children_inserted {
                    __golden_base_children(self, ctx);
                }
                #(#param_dependency_reconcile_statements)*
                #(#param_order_reconcile_statements)*
            }

            #[doc(hidden)]
            pub fn __golden_node_engine_preprocess_inbox(
                &mut self,
                ctx: &mut golden_core::process_ctx::ProcessCtx,
                owner_id: golden_core::node::NodeId,
            ) {
                self.__golden_node_engine_preprocess_inbox_with_base_children(ctx, owner_id, |_, _| {});
            }

            #[doc(hidden)]
            pub fn __golden_node_engine_preprocess_inbox_with_base_children<F>(
                &mut self,
                ctx: &mut golden_core::process_ctx::ProcessCtx,
                owner_id: golden_core::node::NodeId,
                mut __golden_base_children: F,
            )
            where
                F: FnMut(&mut Self, &mut golden_core::process_ctx::ProcessCtx),
            {
                let __golden_node_owner_id = owner_id;
                let mut __golden_base_children_inserted = false;
                let __golden_events = ctx.events.clone();
                for event in &__golden_events {
                    match &event.kind {
                        golden_core::events::EventKind::ParamChanged { param, old_value, new_value } => {
                            let param = *param;
                            let old_value = old_value.clone();
                            let new_value = new_value.clone();
                            self.__golden_node_engine_sync_param_handle_cache(param, &new_value);
                            #(#param_change_callback_statements)*
                            let _ = &old_value;
                        }
                        golden_core::events::EventKind::ChildAdded { parent, child, decl_id } => {
                            let parent = *parent;
                            let child = *child;
                            let decl_id = decl_id.clone();
                            #(#child_added_decl_statements)*
                        }
                        golden_core::events::EventKind::ChildReplaced { parent, old, new, decl_id } => {
                            let parent = *parent;
                            let old = *old;
                            let new = *new;
                            let decl_id = decl_id.clone();
                            #(#child_replaced_decl_statements)*
                        }
                        golden_core::events::EventKind::ChildRemoved { parent, child } => {
                            let parent = *parent;
                            let child = *child;
                            #(#child_removed_statements)*
                        }
                        _ => {}
                    }
                }
                if !__golden_base_children_inserted {
                    __golden_base_children(self, ctx);
                }
                #(#param_dependency_reconcile_statements)*
                #(#param_order_reconcile_statements)*
            }
        }

        #generated_node_impl
        #generated_declared_user_item_node
    }
}

fn expand_impl(expansion: NodeExpansion, mut input: ItemImpl) -> proc_macro2::TokenStream {
    let NodeExpansion {
        node:
            NodeAttr {
                type_name,
                ctor_meta_fields,
                presentation_fields: _,
                via,
                impl_node,
                from_struct,
                scriptable,
                contextualizable,
            },
        item_kind,
        item_menu_path,
    } = expansion;
    if impl_node {
        return Error::new_spanned(input, "`impl_node` is only supported on struct declarations").to_compile_error();
    }

    let Some((_, trait_path, _)) = &input.trait_ else {
        return Error::new_spanned(input, "#[node] on impl requires a trait impl: `impl Node for Type`")
            .to_compile_error();
    };

    let is_node_impl = trait_path.segments.last().is_some_and(|seg| seg.ident == "Node");
    if !is_node_impl {
        return Error::new_spanned(trait_path, "#[node] on impl can only be used with `Node` trait").to_compile_error();
    }

    let resolved_type_name = match type_name.clone() {
        Some(type_name) => type_name,
        None => match infer_type_name_from_impl(&input) {
            Ok(type_name) => type_name,
            Err(err) => return err.to_compile_error(),
        },
    };
    let fallback_default_label = make_label_literal(&resolved_type_name.value());
    let generated_default_label = match ctor_meta_fields.get("label") {
        Some((_, expr)) => quote! { (#expr).into() },
        None if from_struct => quote! { Self::default_label() },
        None => quote! { ::std::string::String::from(#fallback_default_label) },
    };

    let node_data_body = if let Some(path) = via.as_ref() {
        let segments = &path.segments;
        quote! { golden_core::node::ViaTarget::via_node_data(&self.#(#segments).*) }
    } else {
        quote! { &self.node_data }
    };

    let node_data_mut_body = if let Some(path) = via.as_ref() {
        let segments = &path.segments;
        quote! { golden_core::node::ViaTarget::via_node_data_mut(&mut self.#(#segments).*) }
    } else {
        quote! { &mut self.node_data }
    };

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
        input.items.push(parse_quote! {
            fn get_type(&self) -> &str {
                #resolved_type_name
            }
        });
    }

    if from_struct && !has_method(&input, "type_description") {
        input.items.push(parse_quote! {
            fn type_description(&self) -> Option<&str> {
                Self::__golden_node_type_description()
            }
        });
    }

    if from_struct && !has_method(&input, "project_create") {
        input.items.push(parse_quote! {
            fn project_create(node_type: &str) -> Option<Self> {
                Self::__golden_node_project_create(node_type)
            }
        });
    }

    if from_struct && !has_method(&input, "project_encode_data") {
        if let Some(path) = via.as_ref() {
            let segments = &path.segments;
            input.items.push(parse_quote! {
                fn project_encode_data(&self) -> Result<serde_json::Value, String> {
                    let __golden_via_data = golden_core::node::ViaTarget::via_project_encode_data(&self.#(#segments).*)?;
                    let __golden_state_data = Self::__golden_node_project_encode_data(self)?;

                    if __golden_via_data.is_null() && __golden_state_data.is_null() {
                        return Ok(serde_json::Value::Null);
                    }

                    let mut __golden_data = serde_json::Map::new();
                    if !__golden_via_data.is_null() {
                        __golden_data.insert("__golden_via".to_string(), __golden_via_data);
                    }
                    if !__golden_state_data.is_null() {
                        __golden_data.insert("__golden_state".to_string(), __golden_state_data);
                    }
                    Ok(serde_json::Value::Object(__golden_data))
                }
            });
        } else {
            input.items.push(parse_quote! {
                fn project_encode_data(&self) -> Result<serde_json::Value, String> {
                    Self::__golden_node_project_encode_data(self)
                }
            });
        }
    }

    if from_struct && !has_method(&input, "project_decode_data") {
        if let Some(path) = via.as_ref() {
            let segments = &path.segments;
            input.items.push(parse_quote! {
                fn project_decode_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
                    if data.is_null() {
                        golden_core::node::ViaTarget::via_project_decode_data(&mut self.#(#segments).*, &serde_json::Value::Null)?;
                        return Self::__golden_node_project_decode_data(self, &serde_json::Value::Null);
                    }

                    let Some(__golden_object) = data.as_object() else {
                        return Err(format!("node type '{}' expects persisted project data as an object", #resolved_type_name));
                    };

                    let __golden_via_data = __golden_object.get("__golden_via").cloned().unwrap_or(serde_json::Value::Null);
                    let __golden_state_data = __golden_object.get("__golden_state").cloned().unwrap_or(serde_json::Value::Null);

                    golden_core::node::ViaTarget::via_project_decode_data(&mut self.#(#segments).*, &__golden_via_data)?;
                    Self::__golden_node_project_decode_data(self, &__golden_state_data)
                }
            });
        } else {
            input.items.push(parse_quote! {
                fn project_decode_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
                    Self::__golden_node_project_decode_data(self, data)
                }
            });
        }
    }

    if !has_method(&input, "as_any") {
        input.items.push(parse_quote! {
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        });
    }

    if !has_method(&input, "as_any_mut") {
        input.items.push(parse_quote! {
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
        });
    }

    if let Some(ref item_kind) = item_kind {
        if !has_method(&input, "user_item_kind") {
            input.items.push(parse_quote! {
                fn user_item_kind(&self) -> &str {
                    #item_kind
                }
            });
        }

        if !has_method(&input, "is_declared_user_item") {
            input.items.push(parse_quote! {
                fn is_declared_user_item(&self) -> bool {
                    true
                }
            });
        }
    }

    if !has_method(&input, "script_host_policy") {
        if let Some(scriptable_attr) = scriptable.as_ref() {
            let method = build_script_host_policy_method_tokens(scriptable_attr);
            input.items.push(parse_quote! {
                #method
            });
        } else if let Some(path) = via.as_ref() {
            let segments = &path.segments;
            input.items.push(parse_quote! {
                fn script_host_policy(&self) -> Option<golden_core::script::ScriptHostPolicy> {
                    golden_core::node::ViaTarget::via_script_host_policy(&self.#(#segments).*)
                }
            });
        }
    }

    if !has_method(&input, "user_context_host_policy") {
        if let Some(contextualizable_attr) = contextualizable.as_ref() {
            let method = build_user_context_host_policy_method_tokens(contextualizable_attr);
            input.items.push(parse_quote! {
                #method
            });
        } else if let Some(path) = via.as_ref() {
            let segments = &path.segments;
            input.items.push(parse_quote! {
                fn user_context_host_policy(&self) -> Option<golden_core::node::UserContextHostPolicy> {
                    golden_core::node::ViaTarget::via_user_context_host_policy(&self.#(#segments).*)
                }
            });
        }
    }

    let has_user_item_factory_macro = has_define_user_item_factory_methods_macro(&input);

    if !has_user_item_factory_macro
        && !has_method(&input, "user_container_rules")
        && let Some(path) = via.as_ref()
    {
        let segments = &path.segments;
        input.items.push(parse_quote! {
            fn user_container_rules(&self) -> Option<golden_core::node::UserContainerRules> {
                golden_core::node::ViaTarget::via_user_container_rules(&self.#(#segments).*)
            }
        });
    }

    if !has_user_item_factory_macro
        && !has_method(&input, "user_container_accepts_item")
        && let Some(path) = via.as_ref()
    {
        let segments = &path.segments;
        input.items.push(parse_quote! {
            fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
                golden_core::node::ViaTarget::via_user_container_accepts_item(
                    &self.#(#segments).*,
                    item_type,
                    item_kind,
                )
            }
        });
    }

    if !has_user_item_factory_macro
        && !has_method(&input, "user_creatable_items")
        && let Some(path) = via.as_ref()
    {
        let segments = &path.segments;
        input.items.push(parse_quote! {
            fn user_creatable_items(&self) -> Vec<golden_core::node::UserCreatableItem> {
                golden_core::node::ViaTarget::via_user_creatable_items(&self.#(#segments).*)
            }
        });
    }

    if !has_user_item_factory_macro
        && !has_method(&input, "create_user_item")
        && let Some(path) = via.as_ref()
    {
        let segments = &path.segments;
        input.items.push(parse_quote! {
            fn create_user_item(&self, node_type: &str) -> Option<Box<dyn golden_core::node::Node>> {
                golden_core::node::ViaTarget::via_create_user_item_for_host(
                    &self.#(#segments).*,
                    #resolved_type_name,
                    node_type,
                )
            }
        });
    }

    if from_struct && let Err(err) = append_struct_methods_from_helpers(&mut input, via.as_ref()) {
        return err.to_compile_error();
    }

    let generated_item_menu_path = build_item_menu_path_tokens(&item_menu_path);
    let generated_declared_user_item_node = item_kind.as_ref().map(|item_kind| {
        let self_ty = &input.self_ty;
        let generics = &input.generics;
        let (impl_generics, _, where_clause) = generics.split_for_impl();
        quote! {
            impl #impl_generics golden_core::node::DeclaredUserItemNode for #self_ty #where_clause {
                const ITEM_NODE_TYPE: &'static str = #resolved_type_name;
                const ITEM_KIND: &'static str = #item_kind;

                fn item_default_label() -> ::std::string::String {
                    #generated_default_label
                }

                fn item_menu_path() -> ::std::vec::Vec<::std::string::String> {
                    #generated_item_menu_path
                }

                fn create_item() -> Self {
                    Self::create()
                }
            }
        }
    });

    quote! {
        #input
        #generated_declared_user_item_node
    }
}

fn append_struct_methods_from_helpers(input: &mut ItemImpl, via: Option<&DelegatePath>) -> Result<()> {
    for method_name in [
        "engine_child_event_interest_depth",
        "engine_sync_param_handle_cache",
        "engine_on_attached",
        "engine_sync_bound_param_handles",
        "engine_materialize_declared_inbox",
        "engine_preprocess_inbox",
    ] {
        if has_method(input, method_name) {
            return Err(Error::new_spanned(
                &*input,
                format!("`from_struct` generates `{method_name}`; remove the manual method or `from_struct`"),
            ));
        }
    }

    let via_child_event_interest_depth = if let Some(path) = via {
        let segments = &path.segments;
        quote! {
            let __golden_via_depth = golden_core::node::ViaTarget::via_engine_child_event_interest_depth(&self.#(#segments).*, event);
            if __golden_via_depth > __golden_depth {
                __golden_depth = __golden_via_depth;
            }
        }
    } else {
        quote! {}
    };

    let engine_on_attached_body = if let Some(path) = via {
        let segments = &path.segments;
        quote! {
            if Self::__GOLDEN_NODE_HAS_ROOT_BASE_CHILDREN_PLACEHOLDER {
                self.__golden_node_engine_on_attached_with_base_children(
                    ctx,
                    owner_id,
                    |__golden_this, __golden_ctx| {
                        golden_core::node::ViaTarget::via_engine_on_attached(
                            &mut __golden_this.#(#segments).*,
                            __golden_ctx,
                        );
                    },
                );
            } else {
                golden_core::node::ViaTarget::via_engine_on_attached(&mut self.#(#segments).*, ctx);
                self.__golden_node_engine_on_attached(ctx, owner_id);
            }
        }
    } else {
        quote! {
            self.__golden_node_engine_on_attached(ctx, owner_id);
        }
    };

    let via_sync_param_handle_cache = if let Some(path) = via {
        let segments = &path.segments;
        quote! {
            golden_core::node::ViaTarget::via_engine_sync_param_handle_cache(&mut self.#(#segments).*, param, new_value);
        }
    } else {
        quote! {}
    };

    let via_sync_bound_param_handles = if let Some(path) = via {
        let segments = &path.segments;
        quote! {
            golden_core::node::ViaTarget::via_engine_sync_bound_param_handles(&mut self.#(#segments).*, resolve);
        }
    } else {
        quote! {}
    };

    let engine_preprocess_inbox_body = if let Some(path) = via {
        let segments = &path.segments;
        quote! {
            if Self::__GOLDEN_NODE_HAS_NESTED_BASE_CHILDREN_PLACEHOLDER {
                self.__golden_node_engine_preprocess_inbox_with_base_children(
                    ctx,
                    owner_id,
                    |__golden_this, __golden_ctx| {
                        golden_core::node::ViaTarget::via_engine_preprocess_inbox(
                            &mut __golden_this.#(#segments).*,
                            __golden_ctx,
                        );
                    },
                );
            } else {
                golden_core::node::ViaTarget::via_engine_preprocess_inbox(&mut self.#(#segments).*, ctx);
                self.__golden_node_engine_preprocess_inbox(ctx, owner_id);
            }
        }
    } else {
        quote! {
            self.__golden_node_engine_preprocess_inbox(ctx, owner_id);
        }
    };

    let engine_materialize_declared_inbox_body = if let Some(path) = via {
        let segments = &path.segments;
        quote! {
            if Self::__GOLDEN_NODE_HAS_NESTED_BASE_CHILDREN_PLACEHOLDER {
                self.__golden_node_engine_materialize_declared_inbox_with_base_children(
                    ctx,
                    owner_id,
                    |__golden_this, __golden_ctx| {
                        golden_core::node::ViaTarget::via_engine_materialize_declared_inbox(
                            &mut __golden_this.#(#segments).*,
                            __golden_ctx,
                        );
                    },
                );
            } else {
                golden_core::node::ViaTarget::via_engine_materialize_declared_inbox(
                    &mut self.#(#segments).*,
                    ctx,
                );
                self.__golden_node_engine_materialize_declared_inbox(ctx, owner_id);
            }
        }
    } else {
        quote! {
            self.__golden_node_engine_materialize_declared_inbox(ctx, owner_id);
        }
    };

    input.items.push(parse_quote! {
        fn engine_child_event_interest_depth(&self, event: &golden_core::events::Event) -> u32 {
            let mut __golden_depth = self.__golden_node_engine_child_event_interest_depth(event);
            #via_child_event_interest_depth
            __golden_depth
        }
    });

    input.items.push(parse_quote! {
        fn engine_on_attached(&mut self, ctx: &mut golden_core::process_ctx::ProcessCtx) {
            let owner_id = golden_core::node::Node::id(self);
            #engine_on_attached_body
        }
    });

    input.items.push(parse_quote! {
        fn engine_sync_param_handle_cache(
            &mut self,
            param: golden_core::node::NodeId,
            new_value: &golden_core::parameter::ParamValue,
        ) {
            self.__golden_node_engine_sync_param_handle_cache(param, new_value);
            #via_sync_param_handle_cache
        }
    });

    input.items.push(parse_quote! {
        fn engine_sync_bound_param_handles(
            &mut self,
            resolve: &mut dyn FnMut(golden_core::node::NodeId) -> Option<golden_core::parameter::ParamValue>,
        ) {
            self.__golden_node_engine_sync_bound_param_handles(resolve);
            #via_sync_bound_param_handles
        }
    });

    input.items.push(parse_quote! {
        fn engine_materialize_declared_inbox(
            &mut self,
            ctx: &mut golden_core::process_ctx::ProcessCtx,
        ) {
            let owner_id = golden_core::node::Node::id(self);
            #engine_materialize_declared_inbox_body
        }
    });

    input.items.push(parse_quote! {
        fn engine_preprocess_inbox(&mut self, ctx: &mut golden_core::process_ctx::ProcessCtx) {
            let owner_id = golden_core::node::Node::id(self);
            #engine_preprocess_inbox_body
        }
    });

    Ok(())
}

fn extract_doc_comment_literal(attrs: &[Attribute]) -> Option<LitStr> {
    let mut lines = Vec::<String>::new();

    for attr in attrs {
        if !attr.path().is_ident("doc") {
            continue;
        }

        let Meta::NameValue(meta) = &attr.meta else {
            continue;
        };
        let Expr::Lit(ExprLit {
            lit: Lit::Str(value), ..
        }) = &meta.value
        else {
            continue;
        };

        let line = value.value();
        lines.push(line.strip_prefix(' ').unwrap_or(line.as_str()).to_string());
    }

    while lines.first().is_some_and(|line| line.trim().is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }

    if lines.is_empty() {
        return None;
    }

    let description = lines.join("\n");
    if description.trim().is_empty() {
        return None;
    }

    Some(LitStr::new(description.as_str(), Span::call_site()))
}

fn params_child_decl_id(plan: &ParamsPlan, child: ParamsChildRef) -> Option<LitStr> {
    match child {
        ParamsChildRef::Folder(index) => Some(plan.folders[index].decl_id.clone()),
        ParamsChildRef::Param(index) => Some(plan.params[index].decl_id.clone()),
        ParamsChildRef::Node(index) => Some(plan.nodes[index].decl_id.clone()),
        ParamsChildRef::BaseChildren => None,
    }
}

fn previous_decl_ids_for_child(plan: &ParamsPlan, parent_key: &str, current: ParamsChildRef) -> Vec<LitStr> {
    let Some(children) = plan.children_by_parent.get(parent_key) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for child in &children.ordered {
        if *child == current {
            break;
        }
        if *child == ParamsChildRef::BaseChildren {
            out.clear();
            continue;
        }
        if let Some(decl_id) = params_child_decl_id(plan, *child) {
            out.push(decl_id);
        }
    }
    out
}

fn build_declared_prev_sibling_tokens(
    prev_decl_ids: &[LitStr],
    parent_expr: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    if prev_decl_ids.is_empty() {
        return quote!(None);
    }

    quote! {
        {
            let mut __golden_prev_sibling = None;
            if let Some(__golden_snapshot) = ctx.tree_snapshot() {
                #(
                    if let Some(__golden_candidate) = __golden_snapshot.find_child(#parent_expr, #prev_decl_ids) {
                        __golden_prev_sibling = Some(__golden_candidate);
                    }
                )*
            }
            __golden_prev_sibling
        }
    }
}

fn extract_dependency_field_ident(expr: &Expr, parameter_fields: &[Ident]) -> Option<Ident> {
    let Expr::Path(ExprPath { qself: None, path, .. }) = expr else {
        return None;
    };
    let ident = path.get_ident()?.clone();
    parameter_fields
        .iter()
        .any(|candidate| candidate == &ident)
        .then_some(ident)
}

fn build_param_dependency_value_tokens(expr: &Expr, parameter_fields: &[Ident]) -> proc_macro2::TokenStream {
    if let Some(field_ident) = extract_dependency_field_ident(expr, parameter_fields) {
        quote!(golden_core::node::ParameterValueType::to_param_value(self.#field_ident.get_ref().clone()))
    } else {
        quote!(::std::convert::Into::<golden_core::parameter::ParamValue>::into(#expr))
    }
}

fn build_param_dependency_closure_tokens(closure: &syn::ExprClosure) -> Result<proc_macro2::TokenStream> {
    match closure.inputs.len() {
        1 => Ok(quote!((#closure)(self))),
        2 => Ok(quote!((#closure)(self, ctx))),
        _ => Err(Error::new_spanned(
            closure,
            "dependency closures must accept `|node: &Self| ...` or `|node: &Self, ctx: &ProcessCtx| ...`",
        )),
    }
}

fn build_param_dependency_eval_tokens(expr: &Expr, parameter_fields: &[Ident]) -> Result<proc_macro2::TokenStream> {
    match expr {
        Expr::Closure(closure) => build_param_dependency_closure_tokens(closure),
        Expr::Paren(inner) => build_param_dependency_eval_tokens(&inner.expr, parameter_fields),
        Expr::Unary(ExprUnary {
            op: UnOp::Not(_), expr, ..
        }) => {
            let inner = build_param_dependency_eval_tokens(expr, parameter_fields)?;
            Ok(quote!(!(#inner)))
        }
        Expr::Binary(ExprBinary { left, right, op, .. }) => match op {
            BinOp::And(_) => {
                let left = build_param_dependency_eval_tokens(left, parameter_fields)?;
                let right = build_param_dependency_eval_tokens(right, parameter_fields)?;
                Ok(quote!((#left) && (#right)))
            }
            BinOp::Or(_) => {
                let left = build_param_dependency_eval_tokens(left, parameter_fields)?;
                let right = build_param_dependency_eval_tokens(right, parameter_fields)?;
                Ok(quote!((#left) || (#right)))
            }
            BinOp::Eq(_) | BinOp::Ne(_) | BinOp::Lt(_) | BinOp::Le(_) | BinOp::Gt(_) | BinOp::Ge(_) => {
                let lhs_tokens = build_param_dependency_value_tokens(left, parameter_fields);
                let rhs_tokens = build_param_dependency_value_tokens(right, parameter_fields);
                let operator = match op {
                    BinOp::Eq(_) => quote!(golden_core::parameter::ParameterDependencyOperator::Eq),
                    BinOp::Ne(_) => quote!(golden_core::parameter::ParameterDependencyOperator::Ne),
                    BinOp::Lt(_) => quote!(golden_core::parameter::ParameterDependencyOperator::Lt),
                    BinOp::Le(_) => quote!(golden_core::parameter::ParameterDependencyOperator::Le),
                    BinOp::Gt(_) => quote!(golden_core::parameter::ParameterDependencyOperator::Gt),
                    BinOp::Ge(_) => quote!(golden_core::parameter::ParameterDependencyOperator::Ge),
                    _ => unreachable!(),
                };
                Ok(quote! {
                    {
                        let __golden_dep_lhs = #lhs_tokens;
                        let __golden_dep_rhs = #rhs_tokens;
                        golden_core::parameter::dependency_binary_compare(&__golden_dep_lhs, &__golden_dep_rhs, #operator)
                    }
                })
            }
            _ => Err(Error::new_spanned(
                expr,
                "unsupported dependency expression; use comparison operators, boolean combinators, or a closure like `|node: &Self| ...` or `|node: &Self, ctx: &ProcessCtx| ...`",
            )),
        },
        Expr::Path(_) => {
            let value_tokens = build_param_dependency_value_tokens(expr, parameter_fields);
            Ok(quote! {
                {
                    let __golden_dep_value = #value_tokens;
                    golden_core::parameter::dependency_truthy(&__golden_dep_value)
                }
            })
        }
        _ => Err(Error::new_spanned(
            expr,
            "unsupported dependency expression; use comparison operators, boolean combinators, or a closure like `|node: &Self| ...` or `|node: &Self, ctx: &ProcessCtx| ...`",
        )),
    }
}

fn build_plan_prev_sibling_tokens(
    plan: &ParamsPlan,
    parent_key: &str,
    current: ParamsChildRef,
    parent_expr: proc_macro2::TokenStream,
    parameter_fields: &[Ident],
) -> Result<proc_macro2::TokenStream> {
    let Some(children) = plan.children_by_parent.get(parent_key) else {
        return Ok(quote!(None));
    };

    let mut checks = Vec::new();
    for child in &children.ordered {
        if *child == current {
            break;
        }

        match *child {
            ParamsChildRef::Folder(index) => {
                let decl_id = &plan.folders[index].decl_id;
                checks.push(quote! {
                    if let Some(__golden_candidate) = __golden_snapshot.find_child(#parent_expr, #decl_id) {
                        __golden_prev_sibling = Some(__golden_candidate);
                    }
                });
            }
            ParamsChildRef::Node(index) => {
                let decl_id = &plan.nodes[index].decl_id;
                checks.push(quote! {
                    if let Some(__golden_candidate) = __golden_snapshot.find_child(#parent_expr, #decl_id) {
                        __golden_prev_sibling = Some(__golden_candidate);
                    }
                });
            }
            ParamsChildRef::Param(index) => {
                let param = &plan.params[index];
                let decl_id = &param.decl_id;
                if let Some(dependency_expr) = &param.dependency {
                    let predicate = build_param_dependency_eval_tokens(dependency_expr, parameter_fields)?;
                    checks.push(quote! {
                        if #predicate {
                            if let Some(__golden_candidate) = __golden_snapshot.find_child(#parent_expr, #decl_id) {
                                __golden_prev_sibling = Some(__golden_candidate);
                            }
                        }
                    });
                } else {
                    checks.push(quote! {
                        if let Some(__golden_candidate) = __golden_snapshot.find_child(#parent_expr, #decl_id) {
                            __golden_prev_sibling = Some(__golden_candidate);
                        }
                    });
                }
            }
            ParamsChildRef::BaseChildren => {
                checks.clear();
            }
        }
    }

    Ok(quote! {
        {
            let mut __golden_prev_sibling = None;
            if let Some(__golden_snapshot) = ctx.tree_snapshot() {
                #(#checks)*
            }
            __golden_prev_sibling
        }
    })
}

fn build_params_plan_param_create_tokens_with_insert_after(
    plan: &ParamsPlan,
    owner_type_name: &LitStr,
    parent_expr: proc_macro2::TokenStream,
    field_ident: &Ident,
    insert_after: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let (_, param) = plan
        .params
        .iter()
        .enumerate()
        .find(|(_, param)| &param.field == field_ident)
        .expect("parameter field should exist in params plan");
    let ty = &param.ty;
    let label_lit = &param.label;
    let decl_id_lit = &param.decl_id;
    let set_description = param.description.as_ref().map(|description_lit| {
        build_set_declared_description_tokens(quote!(__param_node), owner_type_name, decl_id_lit, description_lit)
    });
    let set_short_name = param.meta.short_name.as_ref().map(|short_name_lit| {
        quote! {
            golden_core::node::Node::node_data_mut(&mut __param_node).meta.short_name =
                ::std::string::String::from(#short_name_lit);
        }
    });
    let set_enabled = param.meta.enabled.as_ref().map(|expr| {
        quote! {
            golden_core::node::Node::node_data_mut(&mut __param_node).meta.enabled = #expr;
        }
    });
    let set_can_be_disabled = param.meta.can_be_disabled.as_ref().map(|expr| {
        quote! {
            golden_core::node::Node::node_data_mut(&mut __param_node).meta.can_be_disabled = #expr;
        }
    });
    let set_tags = param.meta.tags.as_ref().map(|expr| {
        quote! {
            golden_core::node::Node::node_data_mut(&mut __param_node).meta.tags = #expr;
        }
    });
    let set_semantics = param.meta.semantics.as_ref().map(|expr| {
        quote! {
            golden_core::node::Node::node_data_mut(&mut __param_node).meta.semantics = #expr;
        }
    });
    let set_presentation = build_presentation_assignment_tokens(
        quote!(
            golden_core::node::Node::node_data_mut(&mut __param_node)
                .meta
                .presentation
        ),
        param.meta.presentation.as_ref(),
        &param.meta.presentation_fields,
    );
    let set_behaviour = match param.behaviour {
        Some(ParamEventBehaviourSpec::Append) => Some(quote! {
            self.#field_ident.set_event_behaviour(golden_core::parameter::ParameterEventBehaviour::Append);
        }),
        Some(ParamEventBehaviourSpec::Coalesce) => Some(quote! {
            self.#field_ident.set_event_behaviour(golden_core::parameter::ParameterEventBehaviour::Coalesce);
        }),
        None => None,
    };
    let set_range = build_range_constraint_assignment(param.min.as_ref(), param.max.as_ref(), ty);
    let set_read_only = param.read_only.as_ref().map(|expr| {
        quote! {
            __param_node.read_only = #expr;
        }
    });
    let set_widget = param.widget.as_ref().map(|widget| {
        quote! {
            __param_node.ui_hints.widget = Some(::std::string::String::from(#widget));
        }
    });
    let set_step = param.step.as_ref().map(|expr| {
        quote! {
            __param_node.constraints.step = Some((#expr) as f64);
        }
    });
    let set_step_base = param.step_base.as_ref().map(|expr| {
        quote! {
            __param_node.constraints.step_base = Some((#expr) as f64);
        }
    });
    let set_enum_options = param.enum_options.as_ref().map(|expr| {
        quote! {
            __param_node.constraints.enum_options = #expr;
        }
    });
    let set_file_allowed_types = match param.file_allowed_types.as_ref() {
        Some(expr) => match build_file_allowed_types_assignment(expr) {
            Ok(tokens) => Some(tokens),
            Err(err) => return err.to_compile_error(),
        },
        None => None,
    };
    let set_file_allowed_extensions = match param.file_allowed_extensions.as_ref() {
        Some(expr) => match build_file_allowed_extensions_assignment(expr) {
            Ok(tokens) => Some(tokens),
            Err(err) => return err.to_compile_error(),
        },
        None => None,
    };
    let set_constraint_policy = match param.constraint_policy {
        Some(ParamConstraintPolicySpec::ClampAdapt) => Some(quote! {
            __param_node.constraints.policy = golden_core::parameter::ParameterConstraintPolicy::ClampAdapt;
        }),
        Some(ParamConstraintPolicySpec::Reject) => Some(quote! {
            __param_node.constraints.policy = golden_core::parameter::ParameterConstraintPolicy::Reject;
        }),
        None => None,
    };
    let set_reference_root = param.reference_root.as_ref().map(|expr| {
        quote! {
            __param_node.constraints.reference.root = #expr;
        }
    });
    let set_reference_target_kind = param.reference_target_kind.as_ref().map(|expr| {
        quote! {
            __param_node.constraints.reference.target_kind = #expr;
        }
    });
    let set_reference_allowed_node_types = param.reference_allowed_node_types.as_ref().map(|expr| {
        quote! {
            __param_node.constraints.reference.allowed_node_types = #expr;
        }
    });
    let set_reference_allowed_parameter_types = param.reference_allowed_parameter_types.as_ref().map(|expr| {
        quote! {
            __param_node.constraints.reference.allowed_parameter_types = #expr;
        }
    });
    let set_reference_allow_projections = param.reference_allow_projections.as_ref().map(|expr| {
        quote! {
            __param_node.constraints.reference.allow_projections = #expr;
        }
    });
    let set_reference_custom_filter_key = param.reference_custom_filter_key.as_ref().map(|expr| {
        quote! {
            __param_node.constraints.reference.custom_filter_key = #expr;
        }
    });
    let set_reference_default_search_filter = param.reference_default_search_filter.as_ref().map(|expr| {
        quote! {
            __param_node.constraints.reference.default_search_filter = #expr;
        }
    });

    quote! {
        let _: &golden_core::node::ParameterHandle<#ty> = &self.#field_ident;
        #set_behaviour
        {
            let __golden_existing_param = ctx
                .tree_snapshot()
                .and_then(|__golden_snapshot| __golden_snapshot.find_child(#parent_expr, #decl_id_lit));
            if let Some(__golden_existing_param) = __golden_existing_param {
                self.#field_ident.set_node_id(__golden_existing_param);
            } else {
                let __golden_param_already_pending = ctx.edits.pending.iter().any(|request| {
                    matches!(
                        &request.edit,
                        golden_core::edit::Edit::AddNode { parent, node, .. }
                            if *parent == #parent_expr
                                && node.node_data().meta.decl_id.0 == #decl_id_lit
                    )
                });
                if !__golden_param_already_pending {
                    let mut __param_node = golden_core::parameter::Parameter::new(
                        #label_lit,
                        <#ty as golden_core::node::ParameterValueType>::to_param_value(
                            self.#field_ident.get_ref().clone()
                        ),
                        self.#field_ident.change_check().clone(),
                    );
                    __param_node.event_behaviour = self.#field_ident.event_behaviour();
                    #set_read_only
                    #set_widget
                    #set_range
                    #set_step
                    #set_step_base
                    #set_enum_options
                    #set_file_allowed_types
                    #set_file_allowed_extensions
                    #set_constraint_policy
                    #set_reference_root
                    #set_reference_target_kind
                    #set_reference_allowed_node_types
                    #set_reference_allowed_parameter_types
                    #set_reference_allow_projections
                    #set_reference_custom_filter_key
                    #set_reference_default_search_filter
                    golden_core::node::Node::node_data_mut(&mut __param_node).meta.decl_id =
                        golden_core::node::DeclId(::std::string::String::from(#decl_id_lit));
                    #set_description
                    #set_short_name
                    #set_enabled
                    #set_can_be_disabled
                    #set_tags
                    #set_semantics
                    #set_presentation
                    ctx.add_child(#parent_expr, __param_node, #insert_after);
                }
            }
        }
    }
}

fn build_params_plan_param_create_tokens(
    plan: &ParamsPlan,
    owner_type_name: &LitStr,
    parent_key: &str,
    parent_expr: proc_macro2::TokenStream,
    field_ident: &Ident,
) -> proc_macro2::TokenStream {
    let (param_index, _) = plan
        .params
        .iter()
        .enumerate()
        .find(|(_, param)| &param.field == field_ident)
        .expect("parameter field should exist in params plan");
    let insert_after = build_declared_prev_sibling_tokens(
        &previous_decl_ids_for_child(plan, parent_key, ParamsChildRef::Param(param_index)),
        parent_expr.clone(),
    );
    build_params_plan_param_create_tokens_with_insert_after(
        plan,
        owner_type_name,
        parent_expr,
        field_ident,
        insert_after,
    )
}

fn materialize_children_tokens(
    plan: &ParamsPlan,
    owner_type_name: &LitStr,
    parent_key: &str,
    parent_expr: proc_macro2::TokenStream,
) -> Vec<proc_macro2::TokenStream> {
    let mut out = Vec::new();
    let Some(children) = plan.children_by_parent.get(parent_key) else {
        return out;
    };

    let plan_param_fields = plan.params.iter().map(|param| param.field.clone()).collect::<Vec<_>>();

    for child in &children.ordered {
        match *child {
            ParamsChildRef::Folder(folder_index) => {
                let folder = &plan.folders[folder_index];
                let label_lit = &folder.label;
                let decl_id_lit = &folder.decl_id;
                let insert_after = build_declared_prev_sibling_tokens(
                    &previous_decl_ids_for_child(plan, parent_key, ParamsChildRef::Folder(folder_index)),
                    parent_expr.clone(),
                );
                let set_description = folder.description.as_ref().map(|description_lit| {
                    build_set_declared_description_tokens(
                        quote!(__folder_node),
                        owner_type_name,
                        decl_id_lit,
                        description_lit,
                    )
                });
                let set_short_name = folder.meta.short_name.as_ref().map(|short_name_lit| {
                    quote! {
                        golden_core::node::Node::node_data_mut(&mut __folder_node).meta.short_name =
                            ::std::string::String::from(#short_name_lit);
                    }
                });
                let set_enabled = folder.meta.enabled.as_ref().map(|expr| {
                    quote! {
                        golden_core::node::Node::node_data_mut(&mut __folder_node).meta.enabled = #expr;
                    }
                });
                let set_can_be_disabled = folder.meta.can_be_disabled.as_ref().map(|expr| {
                    quote! {
                        golden_core::node::Node::node_data_mut(&mut __folder_node).meta.can_be_disabled = #expr;
                    }
                });
                let set_tags = folder.meta.tags.as_ref().map(|expr| {
                    quote! {
                        golden_core::node::Node::node_data_mut(&mut __folder_node).meta.tags = #expr;
                    }
                });
                let set_semantics = folder.meta.semantics.as_ref().map(|expr| {
                    quote! {
                        golden_core::node::Node::node_data_mut(&mut __folder_node).meta.semantics = #expr;
                    }
                });
                let set_presentation = build_presentation_assignment_tokens(
                    quote!(
                        golden_core::node::Node::node_data_mut(&mut __folder_node)
                            .meta
                            .presentation
                    ),
                    folder.meta.presentation.as_ref(),
                    &folder.meta.presentation_fields,
                );
                let guard = folder_materialization_guard(plan, folder_index);
                if folder.reuse {
                    out.push(quote! {
                        if #guard {
                            let __golden_folder_already_exists = ctx
                                .tree_snapshot()
                                .and_then(|__golden_snapshot| __golden_snapshot.find_child(#parent_expr, #decl_id_lit))
                                .is_some();
                            let __golden_folder_already_pending = ctx.edits.pending.iter().any(|request| {
                                matches!(
                                    &request.edit,
                                    golden_core::edit::Edit::AddNode { parent, node, .. }
                                        if *parent == #parent_expr
                                            && node.node_data().meta.decl_id.0 == #decl_id_lit
                                )
                            });
                            if !__golden_folder_already_exists && !__golden_folder_already_pending {
                                let mut __folder_node = golden_core::node::Folder::new(#label_lit);
                                golden_core::node::Node::node_data_mut(&mut __folder_node).meta.decl_id =
                                    golden_core::node::DeclId(::std::string::String::from(#decl_id_lit));
                                #set_description
                                #set_short_name
                                #set_enabled
                                #set_can_be_disabled
                                #set_tags
                                #set_semantics
                                #set_presentation
                                ctx.add_child(#parent_expr, __folder_node, #insert_after);
                            }
                        }
                    });
                } else {
                    out.push(quote! {
                        if #guard {
                            let __golden_folder_already_exists = ctx
                                .tree_snapshot()
                                .and_then(|__golden_snapshot| __golden_snapshot.find_child(#parent_expr, #decl_id_lit))
                                .is_some();
                            if !__golden_folder_already_exists {
                                let mut __folder_node = golden_core::node::Folder::new(#label_lit);
                                golden_core::node::Node::node_data_mut(&mut __folder_node).meta.decl_id =
                                    golden_core::node::DeclId(::std::string::String::from(#decl_id_lit));
                                #set_description
                                #set_short_name
                                #set_enabled
                                #set_can_be_disabled
                                #set_tags
                                #set_semantics
                                #set_presentation
                                ctx.add_child(#parent_expr, __folder_node, #insert_after);
                            }
                        }
                    });
                }
            }
            ParamsChildRef::Param(param_index) => {
                let param = &plan.params[param_index];
                let field_ident = &param.field;
                let create_param = build_params_plan_param_create_tokens(
                    plan,
                    owner_type_name,
                    parent_key,
                    parent_expr.clone(),
                    field_ident,
                );
                if let Some(dependency_expr) = param.dependency.as_ref() {
                    let dependency_predicate =
                        match build_param_dependency_eval_tokens(dependency_expr, &plan_param_fields) {
                            Ok(tokens) => tokens,
                            Err(err) => return vec![err.to_compile_error()],
                        };
                    out.push(quote! {
                        if #dependency_predicate {
                            if !self.#field_ident.is_bound() {
                                #create_param
                            }
                        }
                    });
                } else {
                    out.push(quote! {
                        if !self.#field_ident.is_bound() {
                            #create_param
                        }
                    });
                }
            }
            ParamsChildRef::Node(node_index) => {
                let node = &plan.nodes[node_index];
                let field_ident = &node.field;
                let ty = &node.ty;
                let label_lit = &node.label;
                let decl_id_lit = &node.decl_id;
                let default_expr = &node.default;
                let set_description = node.description.as_ref().map(|description_lit| {
                    build_set_declared_description_tokens(
                        quote!(__child_node),
                        owner_type_name,
                        decl_id_lit,
                        description_lit,
                    )
                });
                let set_short_name = node.meta.short_name.as_ref().map(|short_name_lit| {
                    quote! {
                        golden_core::node::Node::node_data_mut(&mut __child_node).meta.short_name =
                            ::std::string::String::from(#short_name_lit);
                    }
                });
                let set_enabled = node.meta.enabled.as_ref().map(|expr| {
                    quote! {
                        golden_core::node::Node::node_data_mut(&mut __child_node).meta.enabled = #expr;
                    }
                });
                let set_can_be_disabled = node.meta.can_be_disabled.as_ref().map(|expr| {
                    quote! {
                        golden_core::node::Node::node_data_mut(&mut __child_node).meta.can_be_disabled = #expr;
                    }
                });
                let set_tags = node.meta.tags.as_ref().map(|expr| {
                    quote! {
                        golden_core::node::Node::node_data_mut(&mut __child_node).meta.tags = #expr;
                    }
                });
                let set_semantics = node.meta.semantics.as_ref().map(|expr| {
                    quote! {
                        golden_core::node::Node::node_data_mut(&mut __child_node).meta.semantics = #expr;
                    }
                });
                let set_presentation = build_presentation_assignment_tokens(
                    quote!(
                        golden_core::node::Node::node_data_mut(&mut __child_node)
                            .meta
                            .presentation
                    ),
                    node.meta.presentation.as_ref(),
                    &node.meta.presentation_fields,
                );

                out.push(quote! {
                    self.#field_ident.set_parent(#parent_expr);
                    if !self.#field_ident.is_present() && !self.#field_ident.is_pending_create() {
                        let mut __golden_child_already_exists = false;
                        if let Some(__golden_snapshot) = ctx.tree_snapshot() {
                            if let Some(__golden_existing) = __golden_snapshot.find_child(#parent_expr, #decl_id_lit) {
                                self.#field_ident.bind_existing(__golden_existing);
                                __golden_child_already_exists = true;
                            }
                        }

                        if !__golden_child_already_exists {
                            let _: &golden_core::node::DeclaredNodeHandle<#ty> = &self.#field_ident;
                            let mut __child_node: #ty = (#default_expr);
                            golden_core::node::Node::node_data_mut(&mut __child_node).meta.decl_id =
                                golden_core::node::DeclId(::std::string::String::from(#decl_id_lit));
                            golden_core::node::Node::node_data_mut(&mut __child_node).meta.label =
                                ::std::string::String::from(#label_lit);
                            #set_description
                            #set_short_name
                            #set_enabled
                            #set_can_be_disabled
                            #set_tags
                            #set_semantics
                            #set_presentation
                            self.#field_ident.replace_with_boxed(ctx, ::std::boxed::Box::new(__child_node));
                        }
                    }
                });
            }
            ParamsChildRef::BaseChildren => {
                out.push(quote! {
                    if !__golden_base_children_inserted {
                        __golden_base_children_inserted = true;
                        __golden_base_children(self, ctx);
                    }
                });
            }
        }
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
    let descendant_nodes = plan
        .nodes
        .iter()
        .filter(|node| node.path.len() > folder.path.len() && node.path.starts_with(&folder.path))
        .map(|node| node.field.clone())
        .collect::<Vec<_>>();

    let mut bound_guards = Vec::<proc_macro2::TokenStream>::new();
    for field_ident in descendant_params {
        bound_guards.push(quote!(self.#field_ident.is_bound()));
    }
    for field_ident in descendant_nodes {
        bound_guards.push(quote!(self.#field_ident.is_present() || self.#field_ident.is_pending_create()));
    }

    if bound_guards.is_empty() {
        quote!(true)
    } else {
        quote!(!(#(#bound_guards)||*))
    }
}

fn build_param_callback_dispatch(field_ident: Ident, callback_spec: &ParamCallbackSpec) -> proc_macro2::TokenStream {
    match callback_spec {
        ParamCallbackSpec::Default => {
            let field_name = field_ident.to_string();
            let field_name = field_name.strip_prefix("r#").unwrap_or(&field_name);
            let method_ident = format_ident!("on_{}_change", field_name);
            quote! {
                if self.#field_ident.id() == param {
                    self.#method_ident(ctx, old_value.clone());
                }
            }
        }
        ParamCallbackSpec::Custom(callback_expr) => {
            quote! {
                if self.#field_ident.id() == param {
                    (#callback_expr)(self, ctx, old_value.clone());
                }
            }
        }
    }
}

fn build_range_constraint_assignment(
    min_expr: Option<&Expr>,
    max_expr: Option<&Expr>,
    ty: &Type,
) -> Option<proc_macro2::TokenStream> {
    if min_expr.is_none() && max_expr.is_none() {
        return None;
    }

    let vector_arity = vector_param_arity(ty);
    let min_components = min_expr.and_then(extract_vector_components);
    let max_components = max_expr.and_then(extract_vector_components);
    let has_component_shape = min_components.is_some() || max_components.is_some();

    if vector_arity.is_none() && has_component_shape {
        let source_expr = min_expr.or(max_expr)?;
        return Some(
            Error::new_spanned(
                source_expr,
                "tuple/array range bounds are only supported for Vec2/Vec3 parameter types",
            )
            .to_compile_error(),
        );
    }

    if let Some(arity) = vector_arity
        && has_component_shape
    {
        if let Some(source_expr) = min_expr.filter(|_| min_components.is_none()) {
            return Some(
                Error::new_spanned(
                    source_expr,
                    format!("mixed scalar and component bounds are not supported for Vec{arity}; use scalar min/max or tuple/array min/max consistently"),
                )
                .to_compile_error(),
            );
        }

        if let Some(source_expr) = max_expr.filter(|_| max_components.is_none()) {
            return Some(
                Error::new_spanned(
                    source_expr,
                    format!("mixed scalar and component bounds are not supported for Vec{arity}; use scalar min/max or tuple/array min/max consistently"),
                )
                .to_compile_error(),
            );
        }

        if let (Some(values), Some(source_expr)) = (min_components.as_ref(), min_expr)
            && values.len() != arity
        {
            return Some(
                Error::new_spanned(source_expr, format!("expected {arity} values for Vec{arity} min bound"))
                    .to_compile_error(),
            );
        }

        if let (Some(values), Some(source_expr)) = (max_components.as_ref(), max_expr)
            && values.len() != arity
        {
            return Some(
                Error::new_spanned(source_expr, format!("expected {arity} values for Vec{arity} max bound"))
                    .to_compile_error(),
            );
        }

        let min_tokens = if let Some(values) = min_components {
            quote! { Some(vec![#((#values) as f64),*]) }
        } else {
            quote! { None }
        };

        let max_tokens = if let Some(values) = max_components {
            quote! { Some(vec![#((#values) as f64),*]) }
        } else {
            quote! { None }
        };

        return Some(quote! {
            __param_node.constraints.range =
                golden_core::parameter::RangeConstraint::components(#min_tokens, #max_tokens);
        });
    }

    let min_tokens = if let Some(expr) = min_expr {
        quote! { Some((#expr) as f64) }
    } else {
        quote! { None }
    };

    let max_tokens = if let Some(expr) = max_expr {
        quote! { Some((#expr) as f64) }
    } else {
        quote! { None }
    };

    Some(quote! {
        __param_node.constraints.range =
            golden_core::parameter::RangeConstraint::uniform(#min_tokens, #max_tokens);
    })
}

fn vector_param_arity(ty: &Type) -> Option<usize> {
    match ty {
        Type::Path(path) => {
            let ident = path.path.segments.last()?.ident.to_string();
            match ident.as_str() {
                "Vec2" => Some(2),
                "Vec3" => Some(3),
                _ => None,
            }
        }
        Type::Tuple(tuple) => match tuple.elems.len() {
            2 => Some(2),
            3 => Some(3),
            _ => None,
        },
        _ => None,
    }
}

fn extract_vector_components(expr: &Expr) -> Option<Vec<Expr>> {
    match expr {
        Expr::Tuple(tuple) => Some(tuple.elems.iter().cloned().collect()),
        Expr::Array(array) => Some(array.elems.iter().cloned().collect()),
        Expr::Paren(inner) => extract_vector_components(&inner.expr),
        _ => None,
    }
}

fn take_handle_attrs(
    field: &mut Field,
) -> Result<(Option<syn::Attribute>, Option<syn::Attribute>, Option<syn::Attribute>)> {
    let mut param_attr = None;
    let mut potential_attr = None;
    let mut state_attr = None;
    let mut keep = Vec::with_capacity(field.attrs.len());

    for attr in field.attrs.drain(..) {
        if attr.path().is_ident("param") {
            if param_attr.is_some() {
                return Err(Error::new_spanned(attr, "duplicate `param` field attribute"));
            }
            param_attr = Some(attr);
        } else if attr.path().is_ident("potential_node") {
            if potential_attr.is_some() {
                return Err(Error::new_spanned(attr, "duplicate `potential_node` field attribute"));
            }
            potential_attr = Some(attr);
        } else if attr.path().is_ident("state") {
            if state_attr.is_some() {
                return Err(Error::new_spanned(attr, "duplicate `state` field attribute"));
            }
            state_attr = Some(attr);
        } else {
            keep.push(attr);
        }
    }

    field.attrs = keep;
    Ok((param_attr, potential_attr, state_attr))
}

fn is_named_type(ty: &Type, ident: &str) -> bool {
    let Type::Path(path) = ty else {
        return false;
    };

    path.path.segments.last().is_some_and(|segment| segment.ident == ident)
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

fn make_label_literal(node_type: &str) -> LitStr {
    let trimmed = node_type.strip_suffix("_node").unwrap_or(node_type);
    let label = enum_label_from_variant_id(trimmed);
    LitStr::new(&label, proc_macro2::Span::call_site())
}

fn expr_string_literal(expr: &Expr) -> Option<LitStr> {
    let Expr::Lit(ExprLit {
        lit: Lit::Str(value), ..
    }) = expr
    else {
        return None;
    };
    Some(value.clone())
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

fn build_script_host_policy_method_tokens(scriptable: &ScriptableAttr) -> proc_macro2::TokenStream {
    match scriptable {
        ScriptableAttr::Default => {
            quote! {
                fn script_host_policy(&self) -> Option<golden_core::script::ScriptHostPolicy> {
                    Some(golden_core::script::ScriptHostPolicy::default_scriptable())
                }
            }
        }
        ScriptableAttr::Expr(expr) => {
            quote! {
                fn script_host_policy(&self) -> Option<golden_core::script::ScriptHostPolicy> {
                    golden_core::node::IntoScriptHostPolicyOption::into_script_host_policy_option(#expr)
                }
            }
        }
    }
}

fn build_user_context_host_policy_method_tokens(contextualizable: &ContextualizableAttr) -> proc_macro2::TokenStream {
    match contextualizable {
        ContextualizableAttr::Default => {
            quote! {
                fn user_context_host_policy(&self) -> Option<golden_core::node::UserContextHostPolicy> {
                    Some(golden_core::node::UserContextHostPolicy::default_contextualizable())
                }
            }
        }
        ContextualizableAttr::Expr(expr) => {
            quote! {
                fn user_context_host_policy(&self) -> Option<golden_core::node::UserContextHostPolicy> {
                    golden_core::node::IntoUserContextHostPolicyOption::into_user_context_host_policy_option(#expr)
                }
            }
        }
    }
}

fn has_method(item_impl: &ItemImpl, name: &str) -> bool {
    item_impl.items.iter().any(|item| {
        matches!(
            item,
            ImplItem::Fn(function) if function.sig.ident == name
        )
    })
}

fn has_define_user_item_factory_methods_macro(item_impl: &ItemImpl) -> bool {
    item_impl.items.iter().any(|item| {
        matches!(
            item,
            ImplItem::Macro(item_macro)
                if item_macro
                    .mac
                    .path
                    .segments
                    .last()
                    .is_some_and(|segment| segment.ident == "define_user_item_factory_methods")
        )
    })
}
