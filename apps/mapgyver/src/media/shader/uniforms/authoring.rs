use std::sync::Arc;

use golden_core::{
    edit::{Edit, NodeTree},
    node::{DeclId, Folder, Node, NodeId},
    parameter::{ParamValue, Parameter, ParameterChangeCheck, RangeConstraint},
};
use golden_video::shader::{FragmentUniform, UniformValue, inspect_uniforms};

use crate::{
    app::{AppEngine, AppNode},
    media::{
        authoring,
        protocol::{ChannelReference, ChannelSource, MediaEditResult, ShaderInput},
    },
};

use super::{FOLDER, LIMIT, PREFIX, ShaderUniformRow, children, folder, parameters, value};

struct Definitions {
    parent: NodeId,
    uniforms: Arc<[FragmentUniform]>,
    file: bool,
    revision: Option<uuid::Uuid>,
}

fn definitions(engine: &AppEngine, reference: ChannelReference) -> Result<Definitions, String> {
    let node = authoring::channel_id(engine, reference)?;
    let parent = parameters(engine, node).ok_or("Shader Parameters are missing")?;
    let definition =
        authoring::effective_definition(engine, engine.nodes.get(node).ok_or("Shader channel is missing")?)?;
    let ChannelSource::Shader { input, format, .. } = definition.source else {
        return Err("Expected a Shader channel".into());
    };
    let file = matches!(input, ShaderInput::File { .. });
    let (uniforms, revision) = match input {
        ShaderInput::Embedded { code } if !code.trim().is_empty() => (
            inspect_uniforms(format, &code)
                .map_err(|error| error.to_string())?
                .into(),
            None,
        ),
        ShaderInput::File { file: Some(file) } => {
            let manager = authoring::manager(engine)?;
            let Some(AppNode::MediaManager(manager)) = engine.nodes.get(manager) else {
                return Err("Medias is missing".into());
            };
            manager.shader_files.get(reference, format, &file).map_or_else(
                || (Arc::default(), None),
                |(revision, uniforms)| (uniforms, Some(revision)),
            )
        }
        _ => (Arc::default(), None),
    };
    Ok(Definitions {
        parent,
        uniforms,
        file,
        revision,
    })
}

fn existing(engine: &AppEngine, parent: Option<NodeId>) -> impl Iterator<Item = (&str, &Parameter)> {
    parent
        .into_iter()
        .flat_map(|parent| children(engine, parent))
        .filter_map(|node| {
            let AppNode::Parameter(parameter) = node else {
                return None;
            };
            Some((
                parameter.node_data().meta.decl_id.0.strip_prefix(PREFIX)?,
                parameter.as_ref(),
            ))
        })
}

fn compatible(parameter: &Parameter, uniform: &FragmentUniform) -> bool {
    value(parameter.get())
        .is_some_and(|value| std::mem::discriminant(&value) == std::mem::discriminant(&uniform.default))
}

pub(crate) fn inspect(engine: &AppEngine, reference: ChannelReference) -> Result<MediaEditResult, String> {
    let definitions = definitions(engine, reference)?;
    let folder = folder(engine, definitions.parent);
    let rows = definitions
        .uniforms
        .iter()
        .map(|uniform| {
            let parameter = existing(engine, folder)
                .find(|(name, parameter)| *name == uniform.name && compatible(parameter, uniform))
                .map(|(_, parameter)| parameter.node_data().id);
            ShaderUniformRow {
                label: label(uniform).to_owned(),
                name: uniform.name.clone(),
                parameter,
            }
        })
        .collect();
    Ok(MediaEditResult::ShaderUniforms {
        rows,
        revision: definitions.revision,
    })
}

pub(crate) fn add(
    engine: &mut AppEngine,
    reference: ChannelReference,
    name: Option<&str>,
    revision: Option<uuid::Uuid>,
) -> Result<(), String> {
    let Definitions {
        parent,
        uniforms: definitions,
        file,
        revision: prepared,
    } = definitions(engine, reference)?;
    if file && prepared.is_none() {
        return Err("Shader file inputs are unavailable until the current file is prepared".into());
    }
    if revision != prepared {
        return Err("Shader inputs changed. Choose uniforms from the current list.".into());
    }
    if name.is_some_and(|requested| !definitions.iter().any(|uniform| uniform.name == requested)) {
        return Err("The requested uniform is absent from the current shader".into());
    }
    let existing_folder = folder(engine, parent);
    let current = existing(engine, existing_folder).collect::<std::collections::BTreeMap<_, _>>();
    let mut count = current.len();
    let mut reserved = Vec::<String>::new();
    let mut additions = Vec::new();
    for uniform in definitions
        .iter()
        .filter(|uniform| name.is_none_or(|name| uniform.name == name))
    {
        let previous = current.get(uniform.name.as_str());
        if previous.is_some_and(|parameter| compatible(parameter, uniform)) {
            continue;
        }
        count += usize::from(previous.is_none());
        if count > LIMIT {
            return Err(format!("A channel can expose at most {LIMIT} uniforms"));
        }
        let requested = label(uniform);
        let label = match existing_folder {
            Some(folder) => crate::app::labels::unique(
                engine,
                folder,
                Some(requested.into()),
                &uniform.name,
                &reserved.iter().map(String::as_str).collect::<Vec<_>>(),
            )?,
            None if reserved.iter().any(|label| label == requested) => uniform.name.clone(),
            None => requested.to_owned(),
        };
        reserved.push(label.clone());
        additions.push((parameter(uniform, &label), previous.map(|node| node.node_data().id)));
    }
    if additions.is_empty() {
        return Ok(());
    }
    if let Some(folder) = existing_folder {
        for (tree, previous) in additions {
            if let Some(node) = previous {
                engine.edits.push(Edit::RemoveNode { node });
            }
            engine.edits.push(Edit::AddNodeTree {
                parent: folder,
                prev_sibling: None,
                tree,
            });
        }
    } else {
        let mut folder = Folder::new_schema_owned("Uniforms");
        folder.node_data_mut().meta.decl_id = DeclId(FOLDER.into());
        let mut tree = NodeTree::new(folder);
        tree.children.extend(additions.into_iter().map(|(tree, _)| tree));
        engine.edits.push(Edit::AddNodeTree {
            parent,
            prev_sibling: None,
            tree,
        });
    }
    Ok(())
}

fn label(uniform: &FragmentUniform) -> &str {
    uniform
        .label
        .as_deref()
        .map(str::trim)
        .filter(|label| !label.is_empty() && label.len() <= 128 && !label.contains('\0'))
        .unwrap_or(&uniform.name)
}

fn parameter(uniform: &FragmentUniform, label: &str) -> NodeTree {
    let value = match uniform.default {
        UniformValue::Float(value) => ParamValue::Float(f64::from(value)),
        UniformValue::Int(value) => ParamValue::Int(value),
        UniformValue::Bool(value) => ParamValue::Bool(value),
        UniformValue::Point([x, y]) => ParamValue::Vec2(f64::from(x), f64::from(y)),
        UniformValue::Vector3([x, y, z]) => ParamValue::Vec3(f64::from(x), f64::from(y), f64::from(z)),
        UniformValue::Color([r, g, b, a]) => ParamValue::Color(f64::from(r), f64::from(g), f64::from(b), f64::from(a)),
    };
    let mut parameter = Parameter::new(label, value, ParameterChangeCheck::ValueChange);
    let metadata = &mut parameter.node_data_mut().meta;
    metadata.decl_id = DeclId(format!("{PREFIX}{}", uniform.name));
    metadata.description = Some(uniform.name.clone());
    let scalar = |value| match value {
        Some(UniformValue::Float(value)) => Some(f64::from(value)),
        Some(UniformValue::Int(value)) => Some(f64::from(value)),
        _ => None,
    };
    let components = |value| match value {
        Some(UniformValue::Point(value)) => Some(value.into_iter().map(f64::from).collect()),
        Some(UniformValue::Vector3(value)) => Some(value.into_iter().map(f64::from).collect()),
        Some(UniformValue::Color(value)) => Some(value.into_iter().map(f64::from).collect()),
        _ => None,
    };
    parameter.constraints.range = RangeConstraint::uniform(scalar(uniform.min), scalar(uniform.max))
        .or_else(|| RangeConstraint::components(components(uniform.min), components(uniform.max)));
    NodeTree::new(parameter)
}
