use serde::{Deserialize, Serialize};

use crate::node::{Node, NodeData, NodeId};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ParamValue {
    Trigger(),

    Int(i32),
    Float(f64),
    Str(String),
    Bool(bool),

    Vec2(f64, f64),
    Vec3(f64, f64, f64),
    Color(f64, f64, f64, f64),

    Reference(NodeId),
}

//Implement value coercion
impl ParamValue {
    pub fn as_int(&self) -> Option<i32> {
        match self {
            ParamValue::Int(i) => Some(*i),
            ParamValue::Float(f) => Some(*f as i32),
            ParamValue::Str(s) => s.parse().ok(),
            ParamValue::Bool(b) => Some(if *b { 1 } else { 0 }),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f64> {
        match self {
            ParamValue::Int(i) => Some(*i as f64),
            ParamValue::Float(f) => Some(*f),
            ParamValue::Str(s) => s.parse().ok(),
            ParamValue::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<String> {
        match self {
            ParamValue::Int(i) => Some(i.to_string()),
            ParamValue::Float(f) => Some(f.to_string()),
            ParamValue::Str(s) => Some(s.clone()),
            ParamValue::Bool(b) => Some(b.to_string()),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ParamValue::Int(i) => Some(*i != 0),
            ParamValue::Float(f) => Some(*f != 0.0),
            ParamValue::Str(s) => s.parse().ok(),
            ParamValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_vec2(&self) -> Option<(f64, f64)> {
        match self {
            ParamValue::Int(i) => Some((*i as f64, *i as f64)),
            ParamValue::Float(f) => Some((*f, *f)),
            ParamValue::Str(s) => {
                let parts: Vec<&str> = s.split(',').collect();
                if parts.len() == 2 {
                    if let (Ok(x), Ok(y)) = (parts[0].trim().parse(), parts[1].trim().parse()) {
                        return Some((x, y));
                    }
                }
                None
            }
            ParamValue::Vec2(x, y) => Some((*x, *y)),
            _ => None,
        }
    }

    pub fn as_vec3(&self) -> Option<(f64, f64, f64)> {
        match self {
            ParamValue::Int(i) => Some((*i as f64, *i as f64, *i as f64)),
            ParamValue::Float(f) => Some((*f, *f, *f)),
            ParamValue::Str(s) => {
                let parts: Vec<&str> = s.split(',').collect();
                if parts.len() == 3 {
                    if let (Ok(x), Ok(y), Ok(z)) = (parts[0].trim().parse(), parts[1].trim().parse(), parts[2].trim().parse()) {
                        return Some((x, y, z));
                    }
                }
                None
            }
            ParamValue::Vec3(x, y, z) => Some((*x, *y, *z)),
            ParamValue::Color(r, g, b, _) => Some((*r, *g, *b)),
            _ => None,
        }
    }

    pub fn as_color(&self) -> Option<(f64, f64, f64, f64)> {
        match self {
            ParamValue::Int(i) => Some(((*i as f64 / 255.0), (*i as f64 / 255.0), (*i as f64 / 255.0), 1.0)),
            ParamValue::Float(f) => Some((*f, *f, *f, 1.0)),
            ParamValue::Str(s) => {
                let parts: Vec<&str> = s.split(',').collect();
                if parts.len() == 4 {
                    if let (Ok(r), Ok(g), Ok(b), Ok(a)) = (parts[0].trim().parse(), parts[1].trim().parse(), parts[2].trim().parse(), parts[3].trim().parse()) {
                        return Some((r, g, b, a));
                    }
                }
                None
            }
            ParamValue::Color(r, g, b, a) => Some((*r, *g, *b, *a)),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ParameterChangeCheck {
    ValueChange,
    None,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Parameter {
    node_data: NodeData,
    pub value: ParamValue,
    pub change_check: ParameterChangeCheck,
}

impl Parameter {
    pub fn new(label: &str, value: ParamValue, change_check: ParameterChangeCheck) -> Self {
        Self {
            node_data: NodeData::new(label.to_string()),
            value,
            change_check,
        }
    }

    pub fn set(&mut self, new_value: ParamValue) {
        self.value = new_value;
    }

    pub fn get(&self) -> &ParamValue {
        &self.value
    }
}

impl Node for Parameter {
    fn node_data(&self) -> &crate::node::NodeData {
        &self.node_data
    }

    fn node_data_mut(&mut self) -> &mut crate::node::NodeData {
        &mut self.node_data
    }
}
