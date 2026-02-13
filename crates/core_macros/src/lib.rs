use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{
    parse_macro_input, parse_quote, Error, Fields, Ident, ImplItem, Item, ItemImpl, ItemStruct,
    LitInt, LitStr, Result, Token, Type,
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

    let ctor_fields = fields
        .iter()
        .filter_map(|f| {
            let ident = f.ident.clone()?;
            if ident == "node_data" {
                None
            } else {
                Some((ident, f.ty.clone()))
            }
        })
        .collect::<Vec<_>>();

    let ctor_args = ctor_fields.iter().map(|(ident, ty)| quote!(#ident: #ty));
    let ctor_assignments = ctor_fields.iter().map(|(ident, _)| quote!(#ident));

    quote! {
        #input

        impl #impl_generics #struct_name #ty_generics #where_clause {
            pub fn new(label: impl Into<String> #(, #ctor_args)*) -> Self {
                Self {
                    node_data: golden_core::node::NodeData::new(label.into()),
                    #(#ctor_assignments),*
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

    quote! {
        #input
    }
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
