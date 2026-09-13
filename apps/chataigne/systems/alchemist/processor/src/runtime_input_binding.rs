use chataigne_alchemist::StableRef;
use golden_values::Value as RuntimeValue;

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeInputBinding {
    Constant(RuntimeValue),
    Reference(StableRef),
}
