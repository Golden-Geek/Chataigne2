//! One backend edit converts a built-in Mapping processor to an editable Formula.

use golden_core::{
    edit::{Edit, NodeTree},
    node::{Node, NodeId, NodeReference},
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use super::{
    catalog::FormulaCatalog, find_formula_library, managed_regions_from_snapshot,
    processor_managed_region_decl_id, StateProcessor, FORMULA_EXTERNAL_BUILTIN_TAG_PREFIX,
};

const CONVERSION_WARNING_ID: &str = "mapping_conversion";
const MAPPING_BUILTIN_ID: &str = "chataigne.mapping@1";

impl StateProcessor {
    pub(super) fn convert_mapping_to_formula(&mut self, ctx: &mut ProcessCtx) {
        let result = ctx
            .tree_snapshot_arc()
            .ok_or_else(|| "project snapshot is unavailable".to_owned())
            .and_then(|snapshot| self.prepare_mapping_conversion(snapshot.as_ref()));
        match result {
            Ok((library, tree)) => {
                let formula_uuid = tree.node.node_data().meta.uuid;
                ctx.edits.push(Edit::AddUserItemTree {
                    tree,
                    parent: library,
                    prev_sibling: None,
                });
                ctx.edits.push(Edit::SetParam {
                    node: self.formula.id(),
                    value: ParamValue::Reference(NodeReference::new(formula_uuid)),
                    behaviour: ParameterEventBehaviour::Coalesce,
                });
                ctx.clear_node_warning(self.id(), Some(CONVERSION_WARNING_ID));
            }
            Err(reason) => ctx.set_node_warning_with(
                self.id(),
                Some(CONVERSION_WARNING_ID),
                "Mapping conversion failed",
                Some(&reason),
            ),
        }
    }

    fn prepare_mapping_conversion(
        &self,
        snapshot: &ProcessTreeSnapshot,
    ) -> Result<(NodeId, NodeTree), String> {
        let source = self
            .formula_node(snapshot)
            .ok_or_else(|| "the processor has no available Formula".to_owned())?;
        let source_node = snapshot.node(source).ok_or_else(|| "source Formula is missing".to_owned())?;
        let expected_tag = format!("{FORMULA_EXTERNAL_BUILTIN_TAG_PREFIX}{MAPPING_BUILTIN_ID}");
        if !source_node.tags.iter().any(|tag| tag == &expected_tag) {
            return Err("only the built-in Mapping can be converted".to_owned());
        }
        let formula = super::formula_from_snapshot(snapshot, source)
            .map_err(|error| format!("the Mapping Formula is invalid: {error}"))?;
        for definition in &formula.surface.managed_regions {
            if snapshot
                .find_child_by_decl_id(
                    self.id(),
                    &processor_managed_region_decl_id(definition.id.as_str()),
                )
                .is_none()
            {
                return Err(format!("the Mapping's '{}' region is missing", definition.label));
            }
        }
        managed_regions_from_snapshot(snapshot, self.id(), &formula)
            .ok_or_else(|| "the configured Mapping items are invalid".to_owned())?;
        let library = find_formula_library(snapshot)
            .ok_or_else(|| "the project has no Formula Library".to_owned())?;
        let preferred = format!("{} Formula", self.node_data().meta.label);
        let label = unique_formula_label(snapshot, library, &preferred);
        let tree = FormulaCatalog::project_formula_copy_tree(snapshot, source, &label)
            .map_err(|error| error.to_string())?;
        Ok((library, tree))
    }
}

fn unique_formula_label(snapshot: &ProcessTreeSnapshot, library: NodeId, preferred: &str) -> String {
    let used = snapshot
        .child_ids(library)
        .into_iter()
        .filter_map(|id| snapshot.node(id).map(|node| node.label.as_str()))
        .collect::<std::collections::HashSet<_>>();
    if !used.contains(preferred) {
        return preferred.to_owned();
    }
    for suffix in 2.. {
        let candidate = format!("{preferred} {suffix}");
        if !used.contains(candidate.as_str()) {
            return candidate;
        }
    }
    unreachable!("a finite Formula Library cannot exhaust names")
}
