//! Product schema admission before Golden's recoverable node codec can skip records.
use golden_core::engine::ProjectFile;

pub(crate) fn validate_project_file(project: &ProjectFile) -> Result<(), String> {
    let mut pending = vec![(&project.root, None)];
    while let Some((record, parent_type)) = pending.pop() {
        if [
            crate::matrix::column_node::COLUMN_TYPE,
            crate::matrix::cell_node::CELL_TYPE,
        ]
        .contains(&record.node_type.as_str())
            && record.children.iter().any(|folder| {
                folder.children.iter().any(|parameter| {
                    parameter
                        .meta
                        .decl_id
                        .as_ref()
                        .is_some_and(|id| id.0 == "parameters/transition_override")
                })
            })
        {
            return Err("Transition override belongs to Transition Time enabled metadata; boolean override Parameters are unsupported".into());
        }
        if record.node_type.starts_with("mapgyver.media.channel") && record.node_type != "mapgyver.media.channel.v2" {
            return Err("Unsupported channel schema: Pictures require channel-owned File Parameters (v2)".into());
        }
        if record.node_type.starts_with("mapgyver.media.asset") {
            if record.node_type != super::protocol::ASSET_NODE_TYPE {
                return Err(format!(
                    "Unsupported Picture asset schema '{}': a single typed File requires {}",
                    record.node_type,
                    super::protocol::ASSET_NODE_TYPE
                ));
            }
            if record.children.iter().any(|folder| {
                folder.children.iter().any(|parameter| {
                    parameter.meta.decl_id.as_ref().is_some_and(|id| {
                        matches!(
                            id.0.as_str(),
                            "parameters/locator_kind" | "parameters/project_path" | "parameters/absolute_path"
                        )
                    })
                })
            }) {
                return Err("Picture assets require one File; retired location/path Parameters are unsupported".into());
            }
        }
        if matches!(
            record.node_type.as_str(),
            "mapgyver.media.v8" | "mapgyver.media.reference.v3" | super::commands::NODE_TYPE
        ) && record.children.iter().any(|folder| {
            folder.children.iter().any(|parameter| {
                parameter
                    .meta
                    .decl_id
                    .as_ref()
                    .is_some_and(|id| matches!(id.0.as_str(), "parameters/filter" | "parameters/sampling"))
            })
        }) {
            return Err("Media sampling is fixed to Linear; authored sampling Parameters are unsupported".into());
        }
        if record.meta.decl_id.as_ref().is_some_and(|id| {
            matches!(
                id.0.as_str(),
                "parameters/transition_ms" | "parameters/transition_curve"
            )
        }) {
            return Err("Matrix-wide transition settings are unsupported; duration belongs to each Layer".into());
        }
        for (prefix, current) in [
            ("mapgyver.matrix.layer", crate::matrix::node::LAYER_TYPE),
            ("mapgyver.matrix.group", crate::matrix::node::GROUP_TYPE),
        ] {
            if record.node_type.starts_with(prefix) && record.node_type != current {
                return Err(format!(
                    "Unsupported Matrix row schema '{}': typed Parameters require {current}",
                    record.node_type
                ));
            }
            if record.node_type == current && record.data.as_ref().is_some_and(|data| !data.is_null()) {
                return Err("Matrix row settings require Parameter children; opaque row data is unsupported".into());
            }
        }
        if record.node_type == "mapgyver.app_control.v1"
            && !matches!(
                parent_type,
                Some("mapgyver.matrix.column.v3" | "mapgyver.matrix.column_group.v1" | "mapgyver.matrix.cell.v4")
            )
        {
            return Err(
                "Standalone app-control records are unsupported; settings require a Column, Column Group or Cell"
                    .into(),
            );
        }
        if record.node_type.starts_with("mapgyver.matrix.column")
            && record.node_type != crate::matrix::column_node::COLUMN_TYPE
            && record.node_type != crate::matrix::column_group_node::COLUMN_GROUP_TYPE
        {
            return Err(format!(
                "Unsupported Matrix Column schema '{}': typed Column v3 or Column Group v1 is required",
                record.node_type
            ));
        }
        if record.node_type == "mapgyver.application"
            || (record.node_type.starts_with("mapgyver.app_control")
                && ![
                    "mapgyver.app_control.v1",
                    "mapgyver.app_control.argument.v1",
                    "mapgyver.app_control.environment.v1",
                ]
                .contains(&record.node_type.as_str()))
        {
            return Err(format!(
                "Unsupported app-control schema '{}': settings require typed Parameter children",
                record.node_type
            ));
        }
        if record.node_type.starts_with("mapgyver.matrix.cell")
            && record.node_type != crate::matrix::cell_node::CELL_TYPE
        {
            return Err(format!("Unsupported Matrix Cell schema '{}'", record.node_type));
        }
        if record.node_type == crate::matrix::cell_node::CELL_TYPE
            && record.data.as_ref().is_some_and(|data| !data.is_null())
        {
            return Err("Matrix Cell settings require Parameter children; opaque Cell data is unsupported".into());
        }
        if record.node_type.starts_with("mapgyver.composition.placement")
            && record.node_type != super::composition::PLACEMENT_TYPE
        {
            return Err(format!(
                "Unsupported Composition Placement schema '{}': degree rotation requires {}",
                record.node_type,
                super::composition::PLACEMENT_TYPE
            ));
        }
        if record.node_type == "mapgyver.composition.instance" {
            return Err(
                "Unsupported reference-only Composition instance schema; use child media with degree-based Placement v2".into(),
            );
        }
        if record.node_type.starts_with("mapgyver.media.reference") && record.node_type != super::reference::NODE_TYPE {
            return Err(format!("Unsupported Reference media schema '{}'", record.node_type));
        }
        if (record.node_type.starts_with("mapgyver.media.command") && record.node_type != super::commands::NODE_TYPE)
            || record.node_type == "mapgyver_media_command"
        {
            return Err(format!(
                "Unsupported MapGyver command schema '{}': typed commands require {}",
                record.node_type,
                super::commands::NODE_TYPE
            ));
        }
        if record.node_type == "mapgyver.media"
            || (record.node_type.starts_with("mapgyver.media.v") && record.node_type != "mapgyver.media.v8")
        {
            return Err(format!(
                "Unsupported MapGyver media schema '{}': media-owned settings require mapgyver.media.v8",
                record.node_type
            ));
        }
        pending.extend(
            record
                .children
                .iter()
                .map(|child| (child, Some(record.node_type.as_str()))),
        );
    }
    Ok(())
}
