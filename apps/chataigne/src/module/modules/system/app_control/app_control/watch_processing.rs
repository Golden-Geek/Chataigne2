use super::*;
use app_control_runtime::{PathKind, Report};
use golden_core::edit::{Edit, EditPreconditions, NodeReadGuard};

impl AppControlModule {
    pub(super) fn project_report(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        let Some((report, mut row)) = self.service.report.take() else {
            return;
        };
        if !report.fence.current() {
            return;
        }
        if self.service.structure_pending {
            self.service.report = Some((report, row));
            return;
        }
        self.service.reconnect.reset();
        let mut published = false;
        for _ in 0..8 {
            let total = report.config.apps.len() + report.config.folders.len();
            if row >= total {
                break;
            }
            if row < report.config.apps.len() {
                let entry = &report.config.apps[row];
                if let Some(folder) = self.watched_app_value_folders.get(&entry.item_id).copied() {
                    if self.guard_report_row(
                        ctx,
                        snapshot,
                        &report,
                        entry.item_id,
                        entry.uuid,
                        &entry.label,
                        None,
                        &entry.target_path,
                        folder,
                    ) {
                        let observation = &report.rows.apps[row];
                        published = true;
                        let warning = match observation.kind {
                            PathKind::Missing => {
                                Some(("Application path does not exist.", Some(entry.target_path.as_str())))
                            }
                            PathKind::Directory => Some((
                                "Application target must point to an executable file, not a folder.",
                                Some(entry.target_path.as_str()),
                            )),
                            _ => None,
                        };
                        apply_optional_warning(ctx, entry.item_id, INVALID_TARGET_WARNING_ID, warning);
                        self.apply_app_metrics(ctx, snapshot, folder, &observation.metrics);
                        ctx.edits.retain_payload(report.clone());
                    }
                }
            } else {
                let index = row - report.config.apps.len();
                let entry = &report.config.folders[index];
                if let Some(folder) = self.watched_folder_value_folders.get(&entry.item_id).copied() {
                    if self.guard_report_row(
                        ctx,
                        snapshot,
                        &report,
                        entry.item_id,
                        entry.uuid,
                        &entry.label,
                        entry.target_id,
                        &entry.target_path,
                        folder,
                    ) {
                        let observation = &report.rows.folders[index];
                        published = true;
                        let warning = match observation.kind {
                            PathKind::Missing => {
                                Some(("Watch folder does not exist.", Some(entry.target_path.as_str())))
                            }
                            PathKind::File => Some((
                                "Watch folder target must point to a folder.",
                                Some(entry.target_path.as_str()),
                            )),
                            _ => None,
                        };
                        if let Some(target) = entry.target_id {
                            apply_optional_warning(ctx, target, INVALID_TARGET_WARNING_ID, warning);
                        }
                        self.apply_folder_watch_update(ctx, snapshot, folder, entry, &observation.update);
                        if let Some(callback) = &report.callbacks[index] {
                            ctx.emit_custom_event(callback.event(self.id()));
                        }
                        ctx.edits.retain_payload(report.clone());
                    }
                }
            }
            row += 1;
        }
        if published
            && (report
                .config
                .apps
                .iter()
                .any(|entry| !entry.target_path.trim().is_empty())
                || report
                    .config
                    .folders
                    .iter()
                    .any(|entry| !entry.target_path.trim().is_empty()))
        {
            self.base.emit_incoming_traffic(ctx);
        }
        if row < report.config.apps.len() + report.config.folders.len() {
            self.service.report = Some((report, row));
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn guard_report_row(
        &self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        report: &Report,
        item: NodeId,
        uuid: NodeUuid,
        label: &str,
        target: Option<NodeId>,
        path: &str,
        folder: NodeId,
    ) -> bool {
        let Some(node) = snapshot.node(item) else {
            return false;
        };
        if node.uuid != uuid || node.label != label || !node.enabled {
            return false;
        }
        let Some(parameters) = self.base.parameters_id() else {
            return false;
        };
        let root_key = if target.is_some() {
            "watched_folders_targets"
        } else {
            "watched_apps_targets"
        };
        let Some(watch_root) = snapshot.find_child_by_decl_id(parameters, root_key) else {
            return false;
        };
        if node.parent != Some(watch_root)
            || target.is_some_and(|target| snapshot.node(target).is_none_or(|node| node.parent != Some(item)))
        {
            return false;
        }
        let target_node = target.unwrap_or(item);
        if !matches!(snapshot.node(target_node).and_then(|node|node.param_value.as_ref()),Some(ParamValue::File(text)|ParamValue::Str(text)) if text==path)
        {
            return false;
        }
        let Some(value_folder) = snapshot.node(folder) else {
            return false;
        };
        if value_folder.child_count > 16 {
            return false;
        }
        let mut reads = Vec::with_capacity(32);
        for id in [item, target_node] {
            let Some(read) = NodeReadGuard::capture(snapshot, id) else {
                return false;
            };
            reads.push(read);
        }
        for id in [watch_root, parameters] {
            let Some(read) = NodeReadGuard::capture_structure(snapshot, id) else {
                return false;
            };
            reads.push(read);
        }
        let mut ancestor = Some(folder);
        for _ in 0..8 {
            let Some(id) = ancestor else {
                break;
            };
            let Some(read) = NodeReadGuard::capture_structure(snapshot, id) else {
                return false;
            };
            reads.push(read);
            if id == self.id() {
                ancestor = None;
                break;
            }
            ancestor = snapshot.node(id).and_then(|node| node.parent);
        }
        if ancestor.is_some() {
            return false;
        }
        for id in snapshot.child_ids(folder) {
            let Some(read) = NodeReadGuard::capture_structure(snapshot, id) else {
                return false;
            };
            reads.push(read);
        }
        if let Some(changed) = report
            .config
            .folders
            .iter()
            .find(|entry| entry.item_id == item)
            .and_then(|entry| entry.changed_id)
        {
            let Some(read) = NodeReadGuard::capture_structure(snapshot, changed) else {
                return false;
            };
            reads.push(read);
        }
        ctx.edits.push(Edit::CheckPreconditions {
            preconditions: EditPreconditions::new(reads, report.fence.guards()).unwrap(),
        });
        true
    }
    fn apply_app_metrics(
        &self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        folder: NodeId,
        metrics: &app_control_runtime::WatchedAppMetrics,
    ) {
        self.sync_watched_app_value_constraints(ctx, snapshot, folder, metrics);
        for (key, value) in [
            ("running", ParamValue::Bool(metrics.running)),
            ("target_path", ParamValue::Str(metrics.target_path.clone())),
            ("name", ParamValue::Str(metrics.target_name.clone())),
            ("exists", ParamValue::Bool(metrics.exists)),
            ("uptime_seconds", ParamValue::Float(metrics.uptime_seconds)),
            ("process_count", ParamValue::Int(metrics.process_count)),
            ("main_pid", ParamValue::Int(metrics.main_pid)),
            ("window_opened", ParamValue::Bool(metrics.window_opened)),
            ("window_count", ParamValue::Int(metrics.window_count)),
            ("cpu_ratio", ParamValue::Float(metrics.cpu_ratio)),
            ("memory_mb", ParamValue::Float(metrics.memory_mb)),
            ("virtual_memory_mb", ParamValue::Float(metrics.virtual_memory_mb)),
        ] {
            set_value_param(snapshot, ctx, self.id(), folder, key, value);
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
            system_metrics::sync_float_constraints(ctx, snapshot, virtual_memory_id, Some(0.0), None);
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
            self.id(),
            value_folder_id,
            "path",
            ParamValue::Str(update.path.clone()),
        );
        set_value_param(
            snapshot,
            ctx,
            self.id(),
            value_folder_id,
            "exists",
            ParamValue::Bool(update.exists),
        );
        set_value_param(
            snapshot,
            ctx,
            self.id(),
            value_folder_id,
            "entry_count",
            ParamValue::Int(update.entry_count),
        );
        set_value_param(
            snapshot,
            ctx,
            self.id(),
            value_folder_id,
            "last_event_kind",
            ParamValue::Str(update.last_event_kind()),
        );
        set_value_param(
            snapshot,
            ctx,
            self.id(),
            value_folder_id,
            "last_event_path",
            ParamValue::Str(update.last_event_path()),
        );
        set_value_param(
            snapshot,
            ctx,
            self.id(),
            value_folder_id,
            "created_count",
            ParamValue::Int(saturating_i32(update.created.len())),
        );
        set_value_param(
            snapshot,
            ctx,
            self.id(),
            value_folder_id,
            "modified_count",
            ParamValue::Int(saturating_i32(update.modified.len())),
        );
        set_value_param(
            snapshot,
            ctx,
            self.id(),
            value_folder_id,
            "removed_count",
            ParamValue::Int(saturating_i32(update.removed.len())),
        );
        set_value_param(
            snapshot,
            ctx,
            self.id(),
            value_folder_id,
            "last_changed_ago_seconds",
            ParamValue::Float(last_changed_ago_seconds(update)),
        );

        if !update.has_changes() {
            return;
        }

        if let Some(changed_id) = entry.changed_id {
            ctx.set_param_from(
                self.id(),
                changed_id,
                ParamValue::Trigger(),
                ParameterEventBehaviour::Append,
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
    }
}
