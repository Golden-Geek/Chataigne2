pub struct ParameterData {
    pub value: ParamValue,
}

#[derive(Clone, Debug)]
pub enum ParamValue {
    Trigger(),

    Int(i32),
    Float(f64),
    Str(String),
    Bool(bool),

    Vec2(f64, f64), 
    Vec3(f64, f64, f64),
    Color(f64, f64, f64, f64),
}


