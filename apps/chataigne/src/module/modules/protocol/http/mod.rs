mod callbacks;
mod configuration;
use configuration::HttpSettings;
mod lifecycle;
mod response_service;
mod scripts;
use crate::app::module::common::worker_wake::ModuleWake;
use scripts::http_request_from_script;
mod transport;

use golden_io::ReconnectBackoff;
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use golden_core::{
    engine::NodeExecutionRule,
    events::{CustomEvent, Event},
    logerror, node,
    node::{Node, NodeCreationContext, NodeHandle, NodeId, NodeMetaPatch, NodeScriptDescriptor},
    parameter::{Enum, ParamValue},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::module::common::{
    http::{
        body_looks_like_json, content_type_is_json, http_auth_mode_enum_options, response_script_body_arg,
        response_script_json_arg, HttpHeader, HttpRequestPayload, HTTP_AUTH_NONE, HTTP_COMMAND_NODE_TYPES,
    },
    received_values::service::{ReceivedValueCursor, ReceivedValueProgress},
};

#[cfg(test)]
use self::transport::HttpTransportConfig;
use self::transport::{
    HttpFailure, HttpQueuedRequest, HttpRequestOrigin, HttpResponse, HttpWorkerEvent, PreparedHttpResponse,
};
use lifecycle::HttpTransports;
use response_service::{HttpCallbackDelivery, PendingHttpEvent};

const HTTP_RESULT_WARNING_ID: &str = "http_result_admission";
const HTTP_BASE_WARNING_ID: &str = "http_base_address";
const HTTP_AUTH_DECL_ID: &str = "auth";
const HTTP_RESPONSE_RECEIVED_CALLBACK: &str = "responseReceived";
const HTTP_REQUEST_FAILED_CALLBACK: &str = "requestFailed";
const HTTP_SCRIPT_METHODS: &[&str] = &[
    "request",
    "requestJson",
    "get",
    "head",
    "options",
    "delete",
    "post",
    "postJson",
    "put",
    "putJson",
    "patch",
    "patchJson",
    "uploadFile",
];

#[node("http_module", label = "HTTP")]
#[children(
    folder(connection) {
        [base_children];
    }
    folder(parameters) {
        base_address: String = "https://rickandmortyapi.com/api/".to_string() (
            label = "Base address",
            description = "HTTP or HTTPS base URL used to resolve every request path."
        );
        folder(auth, label = "Auth", enabled = false, can_be_disabled = true, collapsed = true) {
            auth_mode: Enum = HTTP_AUTH_NONE (
                label = "Mode",
                description = "Authentication applied to every request while Auth is enabled.",
                enum_options = http_auth_mode_enum_options()
            );
            username: String = String::new() (
                label = "Username",
                description = "Username used by Basic auth."
            );
            password: String = String::new() (
                label = "Password",
                description = "Password used by Basic auth."
            );
            token: String = String::new() (
                label = "Token",
                description = "Bearer token used by Bearer auth."
            );
            header_name: String = String::new() (
                label = "Header Name",
                description = "Header name used by Custom Header auth."
            );
            header_value: String = String::new() (
                label = "Header Value",
                description = "Header value used by Custom Header auth."
            );
        }
        default_headers: String = String::new() (
            label = "Default Headers",
            description = "Headers applied to every request, one per line as Name: Value.",
            widget = "textarea"
        );
        timeout_ms: i32 = 10000 [1..2147483647] (
            label = "Timeout",
            description = "Maximum request duration in milliseconds.",
            widget = "text"
        );
        retry_count: i32 = 1 [0..10] (
            label = "Retry Count",
            description = "Number of retry attempts for transient network errors and retryable HTTP statuses.",
            widget = "text"
        );
        retry_delay_ms: i32 = 250 [0..60000] (
            label = "Retry Delay",
            description = "Base delay between retry attempts in milliseconds.",
            widget = "text"
        );
        auto_add: bool = true (
            label = "Auto Add",
            description = "Automatically create missing Values nodes from JSON responses."
        );
        [base_children];
    }
    folder(values) {
        [base_children];
    }
)]
pub struct HttpModule {
    base: crate::app::ModuleBase,
    transports: HttpTransports,
    transport_dirty: bool,
    pending_events: VecDeque<PendingHttpEvent>,
    reconnect_backoff: ReconnectBackoff,
    retry_at: Option<Instant>,
    wake: Option<ModuleWake>,
    active_settings: Option<HttpSettings<'static>>,
}

impl HttpModule {
    pub fn create() -> Self {
        Self::new(
            crate::app::ModuleBase::new(),
            HttpTransports::default(),
            true,
            VecDeque::new(),
            ReconnectBackoff::new(Duration::from_millis(250), Duration::from_secs(5)),
            None,
            None,
            None,
        )
    }

    #[cfg(test)]
    pub(crate) fn disable_transport_for_test(&mut self) {
        self.stop_transport();
        self.transport_dirty = false;
    }

    #[cfg(test)]
    fn install_transport_for_test(&mut self, config: HttpTransportConfig) {
        self.transports =
            HttpTransports::with_lifetimes(golden_io::RetirementPool::new(std::num::NonZeroUsize::new(3).unwrap()));
        self.transports
            .configure(
                config,
                self.wake.as_ref().expect("ready HTTP test owner").notifier.clone(),
            )
            .unwrap();
        self.transport_dirty = false;
    }

    #[cfg(test)]
    fn send_for_test(&self, request: HttpQueuedRequest) -> Result<(), String> {
        self.admit_request(request)
    }

    #[cfg(test)]
    pub(crate) fn enqueue_response_for_test(&mut self, response: HttpResponse) {
        let HttpWorkerEvent::Response(response) =
            transport::prepared::prepare(transport::HttpOutcome::Response(response))
        else {
            panic!("test response must fit native result admission")
        };
        self.enqueue_pending_response(response);
        self.request_service();
    }

    #[cfg(test)]
    pub(crate) fn has_pending_responses_for_test(&self) -> bool {
        !self.pending_events.is_empty()
    }

    fn module_enabled(&self, snapshot: &ProcessTreeSnapshot) -> bool {
        snapshot.node(self.id()).map(|node| node.enabled).unwrap_or(false)
    }

    fn refresh_transport(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        self.retry_at = None;
        self.transport_dirty = false;
        self.active_settings = None;

        if !self.module_enabled(snapshot) {
            self.transports.pause_admission();
            self.clear_base_warning(ctx);
            self.base.set_connected(ctx, false);
            return;
        }

        let settings = match HttpSettings::read(self, snapshot)
            .and_then(|settings| settings.prepare().map(|config| (settings, config)))
        {
            Ok(settings) => settings,
            Err(error) => {
                logerror!("Invalid HTTP module configuration: {}", error);
                self.transports.pause_admission();
                self.set_base_warning(ctx, error.as_str());
                self.base.set_connected(ctx, false);
                return;
            }
        };

        let Some(wake) = &self.wake else {
            return;
        };
        let (settings, config) = settings;
        match self.transports.configure(config, wake.notifier.clone()) {
            Ok(()) => {
                self.active_settings = Some(settings.owned());
                self.clear_base_warning(ctx);
                self.base.set_connected(ctx, true);
            }
            Err(error) => {
                logerror!("Failed to start HTTP worker: {}", error);
                self.set_base_warning(ctx, error.as_str());
                self.base.set_connected(ctx, false);
                self.transport_dirty = true;
                self.retry_at = Some(self.reconnect_backoff.schedule(Instant::now()));
            }
        }
    }

    fn drain_transport_events(&mut self, ctx: &mut ProcessCtx) {
        // Preserve a bounded batch when a tree snapshot is unavailable; backpressure stays
        // at the worker boundary instead of accumulating another main-thread response queue.
        if !self.pending_events.is_empty() {
            return;
        }
        let poll = self.transports.poll();
        if poll.active_response {
            self.reconnect_backoff.reset();
        }
        for event in poll.events {
            match event {
                HttpWorkerEvent::Response(response) => {
                    self.log_incoming_response(&response);
                    self.enqueue_pending_response(response);
                    self.base.emit_incoming_traffic(ctx);
                }
                HttpWorkerEvent::Failure(failure) => {
                    self.pending_events.push_back(PendingHttpEvent::Failure(failure));
                }
                HttpWorkerEvent::Rejected(rejection) => {
                    self.pending_events
                        .push_back(PendingHttpEvent::Rejected(rejection.into()));
                }
            }
        }

        if poll.active_ended {
            logerror!("HTTP worker ended; restarting.");
            self.set_base_warning(ctx, "HTTP worker ended. Restarting.");
            self.base.set_connected(ctx, false);
            self.transport_dirty = true;
            self.retry_at = Some(self.reconnect_backoff.schedule(Instant::now()));
        }
    }

    fn enqueue_pending_response(&mut self, response: PreparedHttpResponse) {
        if let Err(error) = &response.received_values {
            logerror!(
                "Failed to parse HTTP response from '{}': {}",
                diagnostic_url(&response.url),
                diagnostic_url(error)
            );
        }
        assert!(
            self.pending_events.len() < transport::HTTP_RESPONSE_CAPACITY,
            "bounded HTTP response batch"
        );
        self.pending_events
            .push_back(PendingHttpEvent::Response(response, ReceivedValueCursor::default()));
    }

    fn service_pending_events(&mut self, ctx: &mut ProcessCtx, snapshot: Option<&ProcessTreeSnapshot>) {
        let values_id = self.base.values_id();
        let module = self.id();
        let progress = match self.pending_events.front_mut() {
            Some(PendingHttpEvent::Response(response, cursor)) => {
                cursor.service(response.messages(), ctx, snapshot, values_id, self.auto_add.get(), true)
            }
            Some(PendingHttpEvent::Failure(_)) => ReceivedValueProgress::Complete,
            Some(PendingHttpEvent::Rejected(delivery)) => {
                if delivery.service(ctx, module) {
                    self.pending_events.pop_front();
                }
                return;
            }
            Some(PendingHttpEvent::Callback(delivery)) => {
                if delivery.service(ctx, module) {
                    self.pending_events.pop_front();
                }
                return;
            }
            None => return,
        };
        if progress != ReceivedValueProgress::Pending {
            let clear_warning = self
                .node_data()
                .meta
                .presentation
                .warning(Some(HTTP_RESULT_WARNING_ID))
                .is_some();
            let event = match self.pending_events.pop_front().expect("completed HTTP event") {
                PendingHttpEvent::Response(response, _) if progress == ReceivedValueProgress::Failed => {
                    logerror!("HTTP response application failed: {}", callbacks::APPLICATION_FAILURE);
                    response.application_failure_callback(self.id())
                }
                PendingHttpEvent::Response(response, _) => response.response_callback(self.id()),
                PendingHttpEvent::Failure(failure) => {
                    logerror!("HTTP request failed: {}", failure.error);
                    failure.callback(self.id())
                }
                PendingHttpEvent::Callback(_) | PendingHttpEvent::Rejected(_) => {
                    unreachable!("delivery serviced above")
                }
            };
            let mut delivery = HttpCallbackDelivery::new(event, clear_warning);
            delivery.service(ctx, module);
            self.pending_events.push_front(PendingHttpEvent::Callback(delivery));
        }
    }

    fn queue_request(
        &self,
        ctx: &mut ProcessCtx,
        payload: HttpRequestPayload,
        origin: HttpRequestOrigin,
    ) -> Result<String, String> {
        let snapshot = ctx
            .tree_snapshot_arc()
            .ok_or("HTTP admission requires an applied tree snapshot")?;
        if !self.module_enabled(&snapshot) {
            return Err("HTTP module is disabled; request was not admitted".into());
        }
        let applied = HttpSettings::read(self, &snapshot)?;
        if self.active_settings.as_ref() != Some(&applied) {
            return Err("HTTP configuration is pending; request was not admitted".into());
        }
        let description = format!("{} {}", payload.method.label(), diagnostic_url(payload.path.trim()));
        self.admit_request(HttpQueuedRequest { origin, payload })?;

        self.base.emit_outgoing_traffic(ctx);
        if self.base.log_outgoing_enabled() {
            golden_core::log!(
                origin = self.id();
                format!("Queued HTTP {description}")
            );
        }

        Ok(format!("Queued HTTP {description}"))
    }

    fn admit_request(&self, request: HttpQueuedRequest) -> Result<(), String> {
        if self.transport_dirty {
            return Err("HTTP configuration is pending; request was not admitted".into());
        }
        self.transports.send(request)
    }

    fn handle_script_http_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Option<Result<(), String>> {
        let request = http_request_from_script(method, args)?;

        Some(request.and_then(|payload| {
            self.queue_request(
                ctx,
                payload,
                HttpRequestOrigin::Script {
                    method: method.to_string(),
                },
            )
            .map(|_| ())
        }))
    }

    fn on_custom_event_inner(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        let Some(request) = crate::app::module_command::decode_module_command_request(&event) else {
            return;
        };
        if request.module_id != self.id() || !HTTP_COMMAND_NODE_TYPES.contains(&request.command_type.as_str()) {
            return;
        }

        let command_id = request.command_id;
        let command_type = request.command_type.clone();
        if let Err(error) = serde_json::from_value::<HttpRequestPayload>(request.payload)
            .map_err(|error| format!("invalid HTTP command payload: {error}"))
            .and_then(|payload| {
                self.queue_request(
                    ctx,
                    payload,
                    HttpRequestOrigin::Command {
                        command_id,
                        command_type,
                    },
                )
            })
        {
            logerror!(format!("Failed to handle HTTP command {:?}: {error}", command_id));
        }
    }

    fn on_param_change_inner(&mut self, snapshot: &ProcessTreeSnapshot, param: NodeId) {
        if self.param_affects_transport(snapshot, param) {
            self.mark_configuration_pending(snapshot);
        }
    }

    fn mark_configuration_pending(&mut self, snapshot: &ProcessTreeSnapshot) {
        if self.transport_dirty
            || !HttpSettings::read(self, snapshot)
                .is_ok_and(|settings| self.active_settings.as_ref() == Some(&settings))
        {
            self.transport_dirty = true;
            self.transports.block_admission();
            self.retry_at = None;
            self.reconnect_backoff.reset();
            self.request_service();
        }
    }

    fn param_affects_transport(&self, snapshot: &ProcessTreeSnapshot, param: NodeId) -> bool {
        (self.base_address.is_bound() && self.base_address.id() == param)
            || (self.default_headers.is_bound() && self.default_headers.id() == param)
            || (self.timeout_ms.is_bound() && self.timeout_ms.id() == param)
            || (self.retry_count.is_bound() && self.retry_count.id() == param)
            || (self.retry_delay_ms.is_bound() && self.retry_delay_ms.id() == param)
            || self
                .auth_node_id(snapshot)
                .is_some_and(|node| is_descendant_or_self(snapshot, param, node))
    }

    fn on_effective_enabled_changed_inner(&mut self, ctx: &mut ProcessCtx, enabled: bool) {
        if enabled {
            self.transport_dirty = true;
            self.transports.block_admission();
            self.retry_at = None;
            self.reconnect_backoff.reset();
        } else {
            self.transports.pause_admission();
            self.clear_base_warning(ctx);
            self.base.set_connected(ctx, false);
            self.transport_dirty = false;
            self.active_settings = None;
            self.retry_at = None;
            if let Some(wake) = &self.wake {
                wake.cancel_deadline();
            }
        }
        self.request_service();
    }

    fn auth_node_id(&self, snapshot: &ProcessTreeSnapshot) -> Option<NodeId> {
        let parameters_id = self.base.parameters_id()?;
        snapshot.find_child_by_decl_id(parameters_id, HTTP_AUTH_DECL_ID)
    }

    fn auth_enabled(&self, snapshot: &ProcessTreeSnapshot) -> Option<bool> {
        let auth_id = self.auth_node_id(snapshot)?;
        snapshot.node(auth_id).map(|node| node.enabled)
    }

    fn refresh_data_capabilities(&mut self, ctx: &mut ProcessCtx) {
        self.base
            .set_data_capabilities(ctx, crate::app::module::ModuleDataCapabilities::new(true, true));
    }

    fn set_base_warning(&self, ctx: &mut ProcessCtx, message: &str) {
        if self.base_address.is_bound() {
            NodeHandle::new(self.base_address.id()).set_warning_with(ctx, Some(HTTP_BASE_WARNING_ID), message, None);
        }
    }

    fn clear_base_warning(&self, ctx: &mut ProcessCtx) {
        if self.base_address.is_bound() {
            NodeHandle::new(self.base_address.id()).clear_warning(ctx, Some(HTTP_BASE_WARNING_ID));
        }
    }

    fn stop_transport(&mut self) {
        self.transports.cancel_all();
        self.active_settings = None;
        self.pending_events.clear();
        self.retry_at = None;
        if let Some(wake) = &self.wake {
            wake.cancel_deadline();
        }
    }

    fn request_service(&self) {
        if let Some(wake) = &self.wake {
            wake.notifier.notify();
        }
    }

    fn arm_service(&self) {
        if !self.pending_events.is_empty() || self.transports.has_pending() {
            self.request_service();
        } else if self.transport_dirty {
            if let Some(wake) = &self.wake {
                wake.later(self.retry_at.map_or(Duration::from_millis(1), |at| {
                    at.saturating_duration_since(Instant::now())
                }));
            }
        }
    }

    fn transport_refresh_due(&self) -> bool {
        self.transport_dirty && self.retry_at.is_none_or(|retry| Instant::now() >= retry)
    }

    fn log_incoming_response(&self, response: &HttpResponse) {
        if !self.base.log_incoming_enabled() {
            return;
        }

        golden_core::log!(
            origin = self.id();
            format!(
                "Received HTTP {} from {} ({} bytes)",
                response.status,
                diagnostic_url(&response.url),
                response.body.len()
            )
        );
    }
}

#[golden_core::item(
    "module",
    node = "http_module",
    via = base,
    from_struct,
    menu_path = ["Network"]
)]
impl Node for HttpModule {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.base.init(ctx);
        self.base.configure_command_tester(ctx, HTTP_COMMAND_NODE_TYPES);
        self.transport_dirty = true;
        crate::app::module::enable_module_authoring(self.node_data_mut());
        self.refresh_data_capabilities(ctx);
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        let source = match ctx.node_waker() {
            Ok(source) => source,
            Err(error) => {
                logerror!("Failed to register HTTP worker wake: {}", error);
                return;
            }
        };
        if !self.wake.as_ref().is_some_and(|wake| wake.matches(&source)) {
            self.stop_transport();
            self.wake = match ModuleWake::new(source) {
                Ok(wake) => Some(wake),
                Err(error) => {
                    logerror!("Failed to register HTTP retry deadline: {}", error);
                    return;
                }
            };
            self.transport_dirty = true;
        }
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        self.refresh_transport(ctx, &snapshot);
        self.arm_service();
    }

    fn on_worker_wake(&mut self, ctx: &mut ProcessCtx) {
        if let Some(wake) = &self.wake {
            wake.cancel_deadline();
        }
        self.drain_transport_events(ctx);
        let snapshot = ctx.tree_snapshot_arc();
        if let Some(snapshot) = snapshot.as_deref() {
            self.refresh_data_capabilities(ctx);
            if self.transport_refresh_due() {
                self.refresh_transport(ctx, snapshot);
            }
        }
        self.service_pending_events(ctx, snapshot.as_deref());
        self.arm_service();
    }

    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.stop_transport();
        self.wake = None;
    }

    fn worker_wake_requires_tree_snapshot(&self) -> bool {
        self.transport_refresh_due()
            || match self.pending_events.front() {
                Some(PendingHttpEvent::Response(response, cursor)) => cursor.requires_snapshot(response.messages()),
                Some(PendingHttpEvent::Failure(_) | PendingHttpEvent::Callback(_) | PendingHttpEvent::Rejected(_)) => {
                    false
                }
                None => false,
            }
    }

    fn needs_update(&self) -> bool {
        false
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::passive()
    }

    fn child_event_interest_depth(&self, _event: &Event) -> u32 {
        u32::MAX
    }

    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        crate::app::module::script_api::descriptor_for_node(self.node_data(), self.get_type(), HTTP_SCRIPT_METHODS)
    }

    fn engine_call_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Result<bool, String> {
        if let Some(result) = self.handle_script_http_method(ctx, method, args) {
            result?;
            return Ok(true);
        }

        self.base.engine_call_script_method(ctx, method, args)
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, old_value: ParamValue) {
        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();
        self.base.emit_script_param_callback(ctx, snapshot, param, &old_value);
        self.on_param_change_inner(snapshot, param);
    }

    fn on_meta_changed(&mut self, ctx: &mut ProcessCtx, node: NodeId, patch: NodeMetaPatch) {
        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        if patch.enabled.is_some()
            && self
                .auth_node_id(snapshot_arc.as_ref())
                .is_some_and(|auth_id| auth_id == node)
        {
            self.mark_configuration_pending(snapshot_arc.as_ref());
        }
    }

    fn on_effective_enabled_changed(&mut self, ctx: &mut ProcessCtx, enabled: bool) {
        self.on_effective_enabled_changed_inner(ctx, enabled);
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        self.on_custom_event_inner(ctx, event);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}

fn is_descendant_or_self(snapshot: &ProcessTreeSnapshot, start: NodeId, ancestor: NodeId) -> bool {
    let mut current = Some(start);
    while let Some(node_id) = current {
        if node_id == ancestor {
            return true;
        }
        current = snapshot.node(node_id).and_then(|node| node.parent);
    }
    false
}

fn diagnostic_url(url: &str) -> &str {
    let end = url.char_indices().nth(256).map_or(url.len(), |(index, _)| index);
    &url[..end]
}

#[cfg(test)]
mod tests;
