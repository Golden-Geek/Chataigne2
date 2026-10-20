use proc_macro2::TokenStream;
use quote::quote;
use syn::{Expr, Ident, ImplItem, ItemImpl, LitStr, parse_quote};

pub(crate) fn add_authored_methods(input: &mut ItemImpl, via: Option<&[Ident]>) {
    let has = |name: &str| {
        input
            .items
            .iter()
            .any(|item| matches!(item, ImplItem::Fn(method) if method.sig.ident == name))
    };
    let custom_codec = has("project_encode_data");
    let capture = !has("engine_capture_authored_data");
    let restore = !has("engine_restore_authored_data");
    if capture {
        let body = if custom_codec {
            quote! { self.project_encode_data().map(golden_core::node::NodeAuthoredData::json) }
        } else if let Some(path) = via {
            quote! {
                let inherited = golden_core::node::ViaTarget::via_capture_authored_data(&self.#(#path).*)?;
                let state = Self::__golden_node_capture_authored_data(self)?;
                Ok(golden_core::node::NodeAuthoredData::inherited(inherited, state))
            }
        } else {
            quote! { Self::__golden_node_capture_authored_data(self).map(golden_core::node::NodeAuthoredData::json) }
        };
        input.items.push(parse_quote! {
            fn engine_capture_authored_data(&self) -> Result<golden_core::node::NodeAuthoredData, String> { #body }
        });
    }
    if restore && !custom_codec {
        let body = if let Some(path) = via {
            quote! {
                let (inherited, state) = data.inherited_parts()?;
                golden_core::node::ViaTarget::via_restore_authored_data(&mut self.#(#path).*, inherited)?;
                Self::__golden_node_project_decode_data(self, state)
            }
        } else {
            quote! { Self::__golden_node_project_decode_data(self, data.as_json()?) }
        };
        input.items.push(parse_quote! {
            fn engine_restore_authored_data(&mut self, data: golden_core::node::NodeAuthoredData) -> Result<(), String> { #body }
        });
    }
}

pub(crate) fn encode(fields: &[(Ident, LitStr, Option<Expr>)], complete: bool) -> TokenStream {
    if fields.is_empty() {
        return quote! { Ok(serde_json::Value::Null) };
    }
    let inserts = fields.iter().map(|(field, name, default)| match (complete, default) {
        (false, Some(default)) => quote! {
            let __golden_current_value = serde_json::to_value(&self.#field)
                .map_err(|err| format!("failed to encode '{}' field: {err}", #name))?;
            let __golden_default_value = serde_json::to_value(&(#default))
                .map_err(|err| format!("failed to encode default for '{}' field: {err}", #name))?;
            if __golden_current_value != __golden_default_value {
                __golden_data.insert(::std::string::ToString::to_string(#name), __golden_current_value);
            }
        },
        _ => quote! {
            __golden_data.insert(
                ::std::string::ToString::to_string(#name),
                serde_json::to_value(&self.#field)
                    .map_err(|err| format!("failed to encode '{}' field: {err}", #name))?,
            );
        },
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
}

pub(crate) fn decode(fields: &[(Ident, LitStr, Option<Expr>)], node_type: &LitStr) -> TokenStream {
    if fields.is_empty() {
        return quote! {
            if data.is_null() { return Ok(()); }
            Err(format!("node type '{}' does not support persisted project data", #node_type))
        };
    }
    let decodes = fields.iter().map(|(field, name, _)| {
        quote! {
            if let Some(value) = __golden_object.get(#name) {
                self.#field = serde_json::from_value(value.clone())
                    .map_err(|err| format!("invalid '{}' field: {err}", #name))?;
            }
        }
    });
    quote! {
        if data.is_null() { return Ok(()); }
        let Some(__golden_object) = data.as_object() else {
            return Err(format!("node type '{}' expects persisted project data as an object", #node_type));
        };
        #(#decodes)*
        Ok(())
    }
}
