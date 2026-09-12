use super::*;

impl AppControlModule {
    pub(super) fn update_watched_app_values(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        watched_apps: &[WatchedAppEntry],
    ) {
        for entry in watched_apps {
            let Some(folder_id) = self.watched_app_value_folders.get(&entry.item_id).copied() else {
                continue;
            };
            if snapshot.node(folder_id).is_none() {
                continue;
            }

            let metrics = self.runtime.watched_app_metrics(entry.target_path.as_str());
            self.sync_watched_app_value_constraints(ctx, snapshot, folder_id, &metrics);
            self.handle_running_control(ctx, snapshot, entry, folder_id, metrics.running);
            set_value_param(
                snapshot,
                ctx,
                folder_id,
                "target_path",
                ParamValue::Str(metrics.target_path),
            );
            set_value_param(
                snapshot,
                ctx,
                folder_id,
                "name",
                ParamValue::Str(metrics.target_name),
            );
            set_value_param(
                snapshot,
                ctx,
                folder_id,
                "exists",
                ParamValue::Bool(metrics.exists),
            );
            set_value_param(
                snapshot,
                ctx,
                folder_id,
                "uptime_seconds",
                ParamValue::Float(metrics.uptime_seconds),
            );
            set_value_param(
                snapshot,
                ctx,
                folder_id,
                "process_count",
                ParamValue::Int(metrics.process_count),
            );
            set_value_param(
                snapshot,
                ctx,
                folder_id,
                "main_pid",
                ParamValue::Int(metrics.main_pid),
            );
            set_value_param(
                snapshot,
                ctx,
                folder_id,
                "window_opened",
                ParamValue::Bool(metrics.window_opened),
            );
            set_value_param(
                snapshot,
                ctx,
                folder_id,
                "window_count",
                ParamValue::Int(metrics.window_count),
            );
            set_value_param(
                snapshot,
                ctx,
                folder_id,
                "cpu_ratio",
                ParamValue::Float(metrics.cpu_ratio),
            );
            set_value_param(
                snapshot,
                ctx,
                folder_id,
                "memory_mb",
                ParamValue::Float(metrics.memory_mb),
            );
            set_value_param(
                snapshot,
                ctx,
                folder_id,
                "virtual_memory_mb",
                ParamValue::Float(metrics.virtual_memory_mb),
            );
        }
    }

    pub(super) fn sync_watched_app_value_constraints(
        &self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        value_folder_id: NodeId,
        metrics: &app_control_runtime::WatchedAppMetrics,
    ) {
        let Some(cpu_id) = find_child_by_key(snapshot, value_folder_id, "cpu_ratio") else {
            return;
        };
        system_metrics::sync_float_constraints(ctx, snapshot, cpu_id, Some(0.0), Some(1.0));

        if let Some(memory_id) = find_child_by_key(snapshot, value_folder_id, "memory_mb") {
            system_metrics::sync_float_constraints(
                ctx,
                snapshot,
                memory_id,
                Some(0.0),
                Some(metrics.memory_max_mb.max(metrics.memory_mb)),
            );
        }

        if let Some(uptime_id) = find_child_by_key(snapshot, value_folder_id, "uptime_seconds") {
            system_metrics::sync_float_constraints(ctx, snapshot, uptime_id, Some(0.0), None);
        }

        if let Some(virtual_memory_id) = find_child_by_key(snapshot, value_folder_id, "virtual_memory_mb") {
            system_metrics::sync_float_constraints(
                ctx,
                snapshot,
                virtual_memory_id,
                Some(0.0),
                None,
            );
        }
    }

    pub(super) fn sync_running_value(
        &mut self,
        snapshot: &ProcessTreeSnapshot,
        ctx: &mut ProcessCtx,
        value_folder_id: NodeId,
        running: bool,
    ) {
        let Some(running_id) = find_child_by_key(snapshot, value_folder_id, "running") else {
            return;
        };
        let Some(current_running) = snapshot
            .node(running_id)
            .and_then(|node| node.param_value.as_ref())
            .and_then(ParamValue::as_bool)
        else {
            return;
        };
        if current_running == running {
            self.ignored_running_value_updates.remove(&running_id);
            return;
        }

        self.ignored_running_value_updates.insert(running_id, running);
        ctx.set_param_with_behaviour(
            running_id,
            ParamValue::Bool(running),
            ParameterEventBehaviour::Coalesce,
        );
    }

    pub(super) fn is_running_control_param(&self, snapshot: &ProcessTreeSnapshot, param: NodeId) -> bool {
        let Some(param_node) = snapshot.node(param) else {
            return false;
        };
        let Some(parent_id) = param_node.parent else {
            return false;
        };
        self.watched_app_value_folders
            .values()
            .any(|folder_id| *folder_id == parent_id)
            && (param_node.decl_id == "running"
                || param_node.decl_id.rsplit('/').next() == Some("running")
                || param_node.short_name == "running")
    }

    pub(super) fn is_ignored_running_sync(&self, snapshot: &ProcessTreeSnapshot, param: NodeId) -> bool {
        let Some(expected) = self.ignored_running_value_updates.get(&param).copied() else {
            return false;
        };
        snapshot
            .node(param)
            .and_then(|node| node.param_value.as_ref())
            .and_then(ParamValue::as_bool)
            == Some(expected)
    }

    pub(super) fn clear_watched_app_value_state(
        &mut self,
        snapshot: &ProcessTreeSnapshot,
        value_folder_id: NodeId,
    ) {
        let Some(running_id) = find_child_by_key(snapshot, value_folder_id, "running") else {
            return;
        };

        self.pending_running_requests.remove(&running_id);
        self.ignored_running_value_updates.remove(&running_id);
        self.requested_running_control_changes.remove(&running_id);
    }

    pub(super) fn handle_running_control(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        entry: &WatchedAppEntry,
        value_folder_id: NodeId,
        actual_running: bool,
    ) {
        let Some(running_id) = find_child_by_key(snapshot, value_folder_id, "running") else {
            return;
        };
        let Some(desired_running) = snapshot
            .node(running_id)
            .and_then(|node| node.param_value.as_ref())
            .and_then(ParamValue::as_bool)
        else {
            return;
        };

        let ignored_sync = self
            .ignored_running_value_updates
            .remove(&running_id)
            .is_some_and(|expected| expected == desired_running);

        if desired_running == actual_running || ignored_sync {
            self.pending_running_requests.remove(&running_id);
            self.requested_running_control_changes.remove(&running_id);
            self.sync_running_value(snapshot, ctx, value_folder_id, actual_running);
            return;
        }

        if self
            .pending_running_requests
            .get(&running_id)
            .is_some_and(|pending| *pending == desired_running)
        {
            return;
        }

        if !self.requested_running_control_changes.remove(&running_id) {
            self.pending_running_requests.remove(&running_id);
            self.sync_running_value(snapshot, ctx, value_folder_id, actual_running);
            return;
        }

        let request_result = if desired_running {
            self.execute_launch_request(
                ctx,
                std::slice::from_ref(entry),
                LaunchProcessRequest {
                    watched_app: entry.label.clone(),
                    executable_path: String::new(),
                    arguments: String::new(),
                    working_directory: String::new(),
                    command_line: String::new(),
                    mode: LaunchMode::WatchedApp,
                },
            )
        } else {
            self.execute_kill_request(
                ctx,
                std::slice::from_ref(entry),
                KillProcessRequest {
                    target_source: CommandTargetSource::WatchedApp,
                    target: entry.label.clone(),
                    match_mode: ProcessMatchMode::Exact,
                    hard_kill: false,
                },
            )
        };

        match request_result {
            Ok(()) => {
                self.pending_running_requests.insert(running_id, desired_running);
            }
            Err(error) => {
                self.pending_running_requests.remove(&running_id);
                self.requested_running_control_changes.remove(&running_id);
                logerror!(format!(
                    "Failed to apply App Control running toggle for '{}': {error}",
                    entry.label,
                ));
                self.sync_running_value(snapshot, ctx, value_folder_id, actual_running);
            }
        }
    }

    pub(super) fn update_watched_folder_values(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        watched_folders: &[WatchedFolderEntry],
    ) {
        for entry in watched_folders {
            let Some(folder_id) = self.watched_folder_value_folders.get(&entry.item_id).copied() else {
                continue;
            };
            if snapshot.node(folder_id).is_none() {
                continue;
            }

            let update = self
                .runtime
                .poll_folder(entry.label.as_str(), entry.target_path.as_str());
            self.apply_folder_watch_update(ctx, snapshot, folder_id, entry, &update);
        }
    }

    pub(super) fn apply_folder_watch_update(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        value_folder_id: NodeId,
        entry: &WatchedFolderEntry,
        update: &FolderWatchUpdate,
    ) {
        set_value_param(
            snapshot,
            ctx,
            value_folder_id,
            "path",
            ParamValue::Str(update.path.clone()),
        );
        set_value_param(
            snapshot,
            ctx,
            value_folder_id,
            "exists",
            ParamValue::Bool(update.exists),
        );
        set_value_param(
            snapshot,
            ctx,
            value_folder_id,
            "entry_count",
            ParamValue::Int(update.entry_count),
        );
        set_value_param(
            snapshot,
            ctx,
            value_folder_id,
            "last_event_kind",
            ParamValue::Str(update.last_event_kind()),
        );
        set_value_param(
            snapshot,
            ctx,
            value_folder_id,
            "last_event_path",
            ParamValue::Str(update.last_event_path()),
        );
        set_value_param(
            snapshot,
            ctx,
            value_folder_id,
            "created_count",
            ParamValue::Int(saturating_i32(update.created.len())),
        );
        set_value_param(
            snapshot,
            ctx,
            value_folder_id,
            "modified_count",
            ParamValue::Int(saturating_i32(update.modified.len())),
        );
        set_value_param(
            snapshot,
            ctx,
            value_folder_id,
            "removed_count",
            ParamValue::Int(saturating_i32(update.removed.len())),
        );
        set_value_param(
            snapshot,
            ctx,
            value_folder_id,
            "last_changed_ago_seconds",
            ParamValue::Float(last_changed_ago_seconds(update)),
        );

        if !update.has_changes() {
            return;
        }

        if let Some(changed_id) = entry.changed_id {
            ctx.set_param_with_behaviour(
                changed_id,
                ParamValue::Trigger(),
                ParameterEventBehaviour::Coalesce,
            );
        }

        if self.base.log_incoming_enabled() {
            golden_core::log!(origin = self.id(); format!(
                "Watch folder '{}' changed: +{} ~{} -{}.",
                entry.label,
                update.created.len(),
                update.modified.len(),
                update.removed.len(),
            ));
        }

        self.emit_watch_folder_changed(ctx, entry, update);
    }

    pub(super) fn emit_watch_folder_changed(
        &self,
        ctx: &mut ProcessCtx,
        entry: &WatchedFolderEntry,
        update: &FolderWatchUpdate,
    ) {
        crate::app::module::script_api::emit_script_callback(
            ctx,
            self.id(),
            WATCH_FOLDER_CHANGED_CALLBACK,
            vec![
                crate::app::module::script_api::node_arg(entry.item_id),
                serde_json::json!({
                    "path": update.path,
                    "exists": update.exists,
                    "entryCount": update.entry_count,
                    "created": update.created,
                    "modified": update.modified,
                    "removed": update.removed,
                    "timestampMs": update.timestamp_ms,
                }),
            ],
        );
    }

    pub(super) fn emit_command_requested(
        &self,
        ctx: &mut ProcessCtx,
        command: &str,
        details: serde_json::Value,
    ) {
        crate::app::module::script_api::emit_script_callback(
            ctx,
            self.id(),
            APP_CONTROL_COMMAND_REQUESTED_CALLBACK,
            vec![serde_json::json!(command), details],
        );
    }

    pub(super) fn emit_command_failed(&self, ctx: &mut ProcessCtx, command: &str, error: &str) {
        crate::app::module::script_api::emit_script_callback(
            ctx,
            self.id(),
            APP_CONTROL_COMMAND_FAILED_CALLBACK,
            vec![serde_json::json!(command), serde_json::json!(error)],
        );
    }

    pub(super) fn execute_launch_request(
        &mut self,
        ctx: &mut ProcessCtx,
        watched_apps: &[WatchedAppEntry],
        request: LaunchProcessRequest,
    ) -> Result<(), String> {
        self.base.emit_outgoing_traffic(ctx);
        let watched_target = if request.mode == LaunchMode::WatchedApp {
            Some(resolve_watched_app_target(watched_apps, request.watched_app.as_str())?)
        } else {
            None
        };

        match self.runtime.execute_launch(&request, watched_target) {
            Ok(outcome) => {
                self.emit_command_requested(
                    ctx,
                    "launch",
                    serde_json::json!({
                        "mode": request.mode.as_str(),
                        "watchedTarget": request.watched_app,
                        "effectiveProgram": outcome.effective_program,
                        "arguments": request.arguments,
                        "workingDirectory": request.working_directory,
                        "commandLine": request.command_line,
                    }),
                );
                if self.base.log_outgoing_enabled() {
                    golden_core::log!(origin = self.id(); format!(
                        "App Control launch: mode='{}', watchedApp='{}', executable='{}', args='{}', cwd='{}', commandLine='{}', effectiveProgram='{}'.",
                        request.mode.as_str(),
                        request.watched_app,
                        request.executable_path,
                        request.arguments,
                        request.working_directory,
                        request.command_line,
                        outcome.effective_program,
                    ));
                }
                Ok(())
            }
            Err(error) => {
                self.emit_command_failed(ctx, "launch", error.as_str());
                Err(error)
            }
        }
    }

    pub(super) fn execute_kill_request(
        &mut self,
        ctx: &mut ProcessCtx,
        watched_apps: &[WatchedAppEntry],
        request: KillProcessRequest,
    ) -> Result<(), String> {
        self.base.emit_outgoing_traffic(ctx);
        let watched_target = if request.target_source == CommandTargetSource::WatchedApp {
            Some(resolve_watched_app_target(watched_apps, request.target.as_str())?)
        } else {
            None
        };

        match self.runtime.execute_kill(&request, watched_target) {
            Ok(process_count) => {
                self.emit_command_requested(
                    ctx,
                    "kill",
                    serde_json::json!({
                        "target": request.target,
                        "targetSource": request.target_source.as_str(),
                        "matchMode": request.match_mode.as_str(),
                        "hardKill": request.hard_kill,
                        "processCount": process_count,
                    }),
                );
                if self.base.log_outgoing_enabled() {
                    golden_core::log!(origin = self.id(); format!(
                        "App Control kill: source='{}', target='{}', matchMode='{}', hardKill={}, processCount={}",
                        request.target_source.as_str(),
                        request.target,
                        request.match_mode.as_str(),
                        request.hard_kill,
                        process_count,
                    ));
                }
                Ok(())
            }
            Err(error) => {
                self.emit_command_failed(ctx, "kill", error.as_str());
                Err(error)
            }
        }
    }

    pub(super) fn execute_window_control_request(
        &mut self,
        ctx: &mut ProcessCtx,
        watched_apps: &[WatchedAppEntry],
        request: WindowControlRequest,
    ) -> Result<(), String> {
        self.base.emit_outgoing_traffic(ctx);
        let watched_target = if request.target_source == CommandTargetSource::WatchedApp {
            Some(resolve_watched_app_target(watched_apps, request.target.as_str())?)
        } else {
            None
        };

        match self.runtime.execute_window_control(&request, watched_target) {
            Ok(window_count) => {
                self.emit_command_requested(
                    ctx,
                    "controlWindow",
                    serde_json::json!({
                        "target": request.target,
                        "targetSource": request.target_source.as_str(),
                        "matchMode": request.match_mode.as_str(),
                        "action": request.action.as_str(),
                        "windowCount": window_count,
                        "x": request.x,
                        "y": request.y,
                        "width": request.width,
                        "height": request.height,
                        "alwaysOnTop": request.always_on_top,
                    }),
                );
                if self.base.log_outgoing_enabled() {
                    golden_core::log!(origin = self.id(); format!(
                        "App Control window: source='{}', target='{}', matchMode='{}', action='{}', x={}, y={}, width={}, height={}, alwaysOnTop={}, windowCount={}",
                        request.target_source.as_str(),
                        request.target,
                        request.match_mode.as_str(),
                        request.action.as_str(),
                        request.x,
                        request.y,
                        request.width,
                        request.height,
                        request.always_on_top,
                        window_count,
                    ));
                }
                Ok(())
            }
            Err(error) => {
                self.emit_command_failed(ctx, "controlWindow", error.as_str());
                Err(error)
            }
        }
    }
}
