use golden_core::{
    events::{Event, EventKind},
    node,
    node::{Node, NodeId},
    parameter::{Enum, ParamValue},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::module::common::{
    http::{
        http_body_mode_enum_options, http_method_enum_options, http_upload_method_enum_options,
        parse_form_field_lines, parse_header_lines, HttpFilePart, HttpMethod, HttpRequestBody,
        HttpRequestPayload, HTTP_BODY_FORM, HTTP_BODY_JSON, HTTP_BODY_NONE, HTTP_BODY_TEXT,
        HTTP_METHOD_GET, HTTP_METHOD_POST, HTTP_REQUEST_COMMAND_NODE_TYPE, HTTP_TEXT_CONTENT_TYPE,
        HTTP_UPLOAD_FILE_COMMAND_NODE_TYPE,
    },
    streaming::parser::decode_text_escape_sequences,
};

macro_rules! command_node_impl {
    ($context:literal) => {
        fn child_event_interest_depth(&self, event: &Event) -> u32 {
            match event.kind {
                EventKind::ParamChanged { .. } => u32::MAX,
                _ => 0,
            }
        }

        fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
            let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
                return;
            };
            let snapshot = snapshot_arc.as_ref();
            if !crate::app::module_command::module_command_triggered(snapshot, self.id(), param) {
                return;
            }

            if let Err(error) = self.request_payload(snapshot).and_then(|payload| {
                crate::app::module_command::emit_module_command_request(
                    ctx,
                    snapshot,
                    self.id(),
                    self.get_type(),
                    &payload,
                )
            }) {
                golden_core::logerror!(format!("Failed to trigger {}: {error}", $context));
            }
        }

        fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
            if !crate::app::module_command::is_command_execute_request(&event, self.id()) {
                return;
            }
            let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
                return;
            };
            let snapshot = crate::app::module_command::command_execute_snapshot(
                &event,
                snapshot_arc.as_ref(),
                self.id(),
            );
            let snapshot = snapshot.as_ref();
            if let Err(error) = self.request_payload(snapshot).and_then(|payload| {
                crate::app::module_command::emit_module_command_request(
                    ctx,
                    snapshot,
                    self.id(),
                    self.get_type(),
                    &payload,
                )
            }) {
                golden_core::logerror!(format!("Failed to execute {}: {error}", $context));
            }
        }
    };
}

#[node("http_request_command", label = "Request")]
#[children(
    method: Enum = HTTP_METHOD_GET (
        label = "Method",
        description = "HTTP method used by this request.",
        enum_options = http_method_enum_options()
    );
    path: String = "character".to_string() (
        label = "Path",
        description = "Relative request path resolved against the module Base address."
    );
    query: String = String::new() (
        label = "Query",
        description = "Optional query string appended to the request URL, without the leading question mark."
    );
    value_path: String = String::new() (
        label = "Value Path",
        description = "Optional Values path where JSON response values are written. Empty derives it from Path."
    );
    headers: String = String::new() (
        label = "Headers",
        description = "Request-specific headers, one per line as Name: Value.",
        widget = "textarea"
    );
    body_mode: Enum = HTTP_BODY_NONE (
        label = "Body Mode",
        description = "How the request body should be encoded.",
        enum_options = http_body_mode_enum_options()
    );
    body: String = String::new() (
        label = "Body",
        description = "Text, JSON, or form fields depending on Body Mode. Form fields use one name=value line per field.",
        widget = "textarea"
    );
    content_type: String = HTTP_TEXT_CONTENT_TYPE.to_string() (
        label = "Content Type",
        description = "Content-Type header used by Text body mode."
    );
)]
pub struct HttpRequestCommand {
    base: crate::app::ModuleCommandBase,
}

impl HttpRequestCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }

    fn request_payload(&self, snapshot: &ProcessTreeSnapshot) -> Result<HttpRequestPayload, String> {
        let method_variant = command_enum_param(snapshot, self.id(), "method")
            .unwrap_or_else(|| HTTP_METHOD_GET.to_string());
        let method = HttpMethod::from_variant(method_variant.as_str())
            .ok_or_else(|| format!("invalid HTTP method variant '{method_variant}'"))?;
        let body_mode = command_enum_param(snapshot, self.id(), "body_mode")
            .unwrap_or_else(|| HTTP_BODY_NONE.to_string());

        Ok(HttpRequestPayload {
            method,
            path: command_string_param(snapshot, self.id(), "path").unwrap_or_else(|| "/".to_string()),
            query: command_string_param(snapshot, self.id(), "query").unwrap_or_default(),
            value_path: command_string_param(snapshot, self.id(), "value_path").unwrap_or_default(),
            headers: parse_header_lines(
                command_string_param(snapshot, self.id(), "headers")
                    .unwrap_or_default()
                    .as_str(),
                "HTTP command",
            )?,
            body: command_body(
                body_mode.as_str(),
                command_string_param(snapshot, self.id(), "body")
                    .unwrap_or_default()
                    .as_str(),
                command_string_param(snapshot, self.id(), "content_type")
                    .unwrap_or_else(|| HTTP_TEXT_CONTENT_TYPE.to_string())
                    .as_str(),
            )?,
            description: format!("{} request", method.label()),
        })
    }
}

#[golden_core::item("module_command", node = "http_request_command", via = base, from_struct)]
impl Node for HttpRequestCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == HTTP_REQUEST_COMMAND_NODE_TYPE).then(Self::create)
    }

    command_node_impl!("HTTP request command");
}

#[node("http_upload_file_command", label = "Upload File")]
#[children(
    method: Enum = HTTP_METHOD_POST (
        label = "Method",
        description = "HTTP method used by this multipart upload.",
        enum_options = http_upload_method_enum_options()
    );
    path: String = "/upload".to_string() (
        label = "Path",
        description = "Relative upload path resolved against the module Base address."
    );
    query: String = String::new() (
        label = "Query",
        description = "Optional query string appended to the upload URL, without the leading question mark."
    );
    value_path: String = String::new() (
        label = "Value Path",
        description = "Optional Values path where JSON response values are written. Empty derives it from Path."
    );
    headers: String = String::new() (
        label = "Headers",
        description = "Upload-specific headers, one per line as Name: Value.",
        widget = "textarea"
    );
    file: golden_core::parameter::File = golden_core::parameter::File::default() (
        label = "File",
        description = "File sent as a multipart/form-data part."
    );
    field_name: String = "file".to_string() (
        label = "Field Name",
        description = "Multipart field name used for the uploaded file."
    );
    file_name: String = String::new() (
        label = "File Name",
        description = "Optional multipart filename override. Empty uses the selected file name."
    );
    content_type: String = String::new() (
        label = "Content Type",
        description = "Optional MIME type override for the file part."
    );
    form_fields: String = String::new() (
        label = "Form Fields",
        description = "Additional multipart text fields, one per line as name=value.",
        widget = "textarea"
    );
)]
pub struct HttpUploadFileCommand {
    base: crate::app::ModuleCommandBase,
}

impl HttpUploadFileCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }

    fn request_payload(&self, snapshot: &ProcessTreeSnapshot) -> Result<HttpRequestPayload, String> {
        let method_variant = command_enum_param(snapshot, self.id(), "method")
            .unwrap_or_else(|| HTTP_METHOD_POST.to_string());
        let method = HttpMethod::from_variant(method_variant.as_str())
            .ok_or_else(|| format!("invalid HTTP upload method variant '{method_variant}'"))?;
        if !matches!(method, HttpMethod::Post | HttpMethod::Put | HttpMethod::Patch) {
            return Err("HTTP upload command only supports POST, PUT, and PATCH".to_string());
        }

        let file_path = command_string_param(snapshot, self.id(), "file").unwrap_or_default();
        if file_path.trim().is_empty() {
            return Err("HTTP upload command requires a file path".to_string());
        }

        let field_name = command_string_param(snapshot, self.id(), "field_name")
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "file".to_string());
        let file_name = command_string_param(snapshot, self.id(), "file_name")
            .filter(|value| !value.trim().is_empty());
        let content_type = command_string_param(snapshot, self.id(), "content_type")
            .filter(|value| !value.trim().is_empty());

        Ok(HttpRequestPayload {
            method,
            path: command_string_param(snapshot, self.id(), "path").unwrap_or_else(|| "/upload".to_string()),
            query: command_string_param(snapshot, self.id(), "query").unwrap_or_default(),
            value_path: command_string_param(snapshot, self.id(), "value_path").unwrap_or_default(),
            headers: parse_header_lines(
                command_string_param(snapshot, self.id(), "headers")
                    .unwrap_or_default()
                    .as_str(),
                "HTTP upload command",
            )?,
            body: HttpRequestBody::Multipart {
                fields: parse_form_field_lines(
                    command_string_param(snapshot, self.id(), "form_fields")
                        .unwrap_or_default()
                        .as_str(),
                    "HTTP upload command",
                )?,
                files: vec![HttpFilePart {
                    field_name,
                    path: file_path,
                    file_name,
                    content_type,
                }],
            },
            description: format!("{} file upload", method.label()),
        })
    }
}

#[golden_core::item(
    "module_command",
    node = "http_upload_file_command",
    via = base,
    from_struct
)]
impl Node for HttpUploadFileCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == HTTP_UPLOAD_FILE_COMMAND_NODE_TYPE).then(Self::create)
    }

    command_node_impl!("HTTP upload command");
}

fn command_body(body_mode: &str, body: &str, content_type: &str) -> Result<HttpRequestBody, String> {
    match body_mode {
        HTTP_BODY_NONE => Ok(HttpRequestBody::Empty),
        HTTP_BODY_TEXT => Ok(HttpRequestBody::Text {
            text: decode_text_escape_sequences(body),
            content_type: if content_type.trim().is_empty() {
                HTTP_TEXT_CONTENT_TYPE.to_string()
            } else {
                content_type.trim().to_string()
            },
        }),
        HTTP_BODY_JSON => {
            let value = if body.trim().is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::from_str(body)
                    .map_err(|error| format!("invalid HTTP command JSON body: {error}"))?
            };
            Ok(HttpRequestBody::Json { value })
        }
        HTTP_BODY_FORM => Ok(HttpRequestBody::Form {
            fields: parse_form_field_lines(body, "HTTP command form")?,
        }),
        other => Err(format!("invalid HTTP body mode '{other}'")),
    }
}

fn command_string_param(snapshot: &ProcessTreeSnapshot, command_id: NodeId, path: &str) -> Option<String> {
    crate::app::module_command::resolve_module_command_child(snapshot, command_id, path).and_then(|param_id| {
        snapshot
            .node(param_id)
            .and_then(|node| node.param_value.as_ref())
            .and_then(ParamValue::as_str)
    })
}

fn command_enum_param(snapshot: &ProcessTreeSnapshot, command_id: NodeId, path: &str) -> Option<String> {
    crate::app::module_command::resolve_module_command_child(snapshot, command_id, path).and_then(|param_id| {
        snapshot
            .node(param_id)
            .and_then(|node| node.param_value.as_ref())
            .and_then(ParamValue::as_enum)
    })
}

#[cfg(test)]
mod tests;
