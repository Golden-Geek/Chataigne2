//! Parameter values, constraints, projections, references, and control contracts.

mod canonical;
mod constraints;
mod control;
mod projection;
mod reference;
mod types;
mod value;
mod value_type;

pub use canonical::*;
pub use constraints::*;
pub use control::*;
pub use golden_values::ColorValue as Color;
pub use projection::*;
pub use reference::*;
pub use types::*;
pub use value::*;
pub use value_type::*;

#[cfg(test)]
mod tests;
