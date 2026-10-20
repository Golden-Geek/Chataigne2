//! Parameter values, constraints, projections, references, and control contracts.

mod canonical;
mod coercion;
mod constraints;
mod control;
mod enum_options;
mod projection;
mod reference;
mod retained;
mod types;
mod value;
mod value_type;

pub use canonical::*;
pub use coercion::*;
pub use constraints::*;
pub use control::*;
pub use enum_options::*;
pub use golden_values::ColorValue as Color;
pub use projection::*;
pub use reference::*;
pub use types::*;
pub use value::*;
pub use value_type::*;

#[cfg(test)]
mod tests;
