use std::sync::Arc;

use indexmap::IndexMap;

use crate::{
    ANodeDeclaration, ANodeTypeId, FacetId, RuntimeValue, SurfaceItemKind, TriggerValue, ValueStorageKind, ValueTypeId,
    value::ColorValue,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ConversionKind {
    NonLossy,
    Lossy,
    ScalarBroadcast,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ConversionRule {
    pub target: ValueTypeId,
    pub kind: ConversionKind,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ValueTypeUiDescriptor {
    pub editor: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
}

pub type RuntimeValueFactory = Arc<dyn Fn() -> RuntimeValue + Send + Sync>;

#[derive(Clone)]
pub struct ValueTypeDescriptor {
    pub id: ValueTypeId,
    pub label: String,
    pub storage: ValueStorageKind,
    pub facets: Vec<FacetId>,
    pub conversions: Vec<ConversionRule>,
    pub default_value: RuntimeValueFactory,
    pub ui: ValueTypeUiDescriptor,
}

impl ValueTypeDescriptor {
    #[must_use]
    pub fn new(
        id: ValueTypeId,
        label: impl Into<String>,
        storage: ValueStorageKind,
        default_value: impl Fn() -> RuntimeValue + Send + Sync + 'static,
    ) -> Self {
        Self {
            id,
            label: label.into(),
            storage,
            facets: Vec::new(),
            conversions: Vec::new(),
            default_value: Arc::new(default_value),
            ui: ValueTypeUiDescriptor::default(),
        }
    }

    #[must_use]
    pub fn with_facets(mut self, facets: impl IntoIterator<Item = FacetId>) -> Self {
        self.facets.extend(facets);
        self
    }

    #[must_use]
    pub fn with_conversion(mut self, target: ValueTypeId, kind: ConversionKind) -> Self {
        self.conversions.push(ConversionRule { target, kind });
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FacetDescriptor {
    pub id: FacetId,
    pub label: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RegistryError {
    #[error("value type `{0}` is already registered")]
    DuplicateValueType(ValueTypeId),
    #[error("facet `{0}` is already registered")]
    DuplicateFacet(FacetId),
    #[error("ANode type `{0}` is already registered")]
    DuplicateANode(ANodeTypeId),
}

#[derive(Clone, Default)]
pub struct ValueTypeRegistry {
    descriptors: IndexMap<ValueTypeId, ValueTypeDescriptor>,
}

impl ValueTypeRegistry {
    #[must_use]
    pub fn with_primitives() -> Self {
        let mut registry = Self::default();
        for descriptor in primitive_descriptors() {
            registry
                .register(descriptor)
                .expect("primitive value type IDs must be unique");
        }
        registry
    }

    pub fn register(&mut self, descriptor: ValueTypeDescriptor) -> Result<(), RegistryError> {
        if self.descriptors.contains_key(&descriptor.id) {
            return Err(RegistryError::DuplicateValueType(descriptor.id));
        }
        self.descriptors.insert(descriptor.id.clone(), descriptor);
        Ok(())
    }

    #[must_use]
    pub fn get(&self, id: &ValueTypeId) -> Option<&ValueTypeDescriptor> {
        self.descriptors.get(id)
    }

    #[must_use]
    pub fn contains(&self, id: &ValueTypeId) -> bool {
        self.descriptors.contains_key(id)
    }

    #[must_use]
    pub fn supports_facet(&self, value_type: &ValueTypeId, facet: &FacetId) -> bool {
        self.get(value_type)
            .is_some_and(|descriptor| descriptor.facets.contains(facet))
    }

    #[must_use]
    pub fn can_convert_automatically(&self, from: &ValueTypeId, to: &ValueTypeId) -> bool {
        from == to
            || self
                .get(from)
                .is_some_and(|descriptor| descriptor.conversions.iter().any(|rule| &rule.target == to))
    }

    #[must_use]
    pub fn default_value(&self, id: &ValueTypeId) -> Option<RuntimeValue> {
        self.get(id).map(|descriptor| (descriptor.default_value)())
    }

    pub fn convert_automatically(&self, value: &RuntimeValue, target: &ValueTypeId) -> Result<RuntimeValue, String> {
        let source = value.value_type();
        if !self.can_convert_automatically(&source, target) {
            return Err(format!("cannot convert `{source}` to `{target}`"));
        }
        value.convert_to(target)
    }

    pub fn iter(&self) -> impl Iterator<Item = &ValueTypeDescriptor> {
        self.descriptors.values()
    }
}

#[derive(Default)]
pub struct FacetRegistry {
    descriptors: IndexMap<FacetId, FacetDescriptor>,
}

impl FacetRegistry {
    pub fn register(&mut self, descriptor: FacetDescriptor) -> Result<(), RegistryError> {
        if self.descriptors.contains_key(&descriptor.id) {
            return Err(RegistryError::DuplicateFacet(descriptor.id));
        }
        self.descriptors.insert(descriptor.id.clone(), descriptor);
        Ok(())
    }

    #[must_use]
    pub fn get(&self, id: &FacetId) -> Option<&FacetDescriptor> {
        self.descriptors.get(id)
    }
}

#[derive(Clone, Default)]
pub struct ANodeRegistry {
    declarations: IndexMap<ANodeTypeId, Arc<dyn ANodeDeclaration>>,
}

impl ANodeRegistry {
    pub fn register(&mut self, declaration: impl ANodeDeclaration + 'static) -> Result<(), RegistryError> {
        self.register_shared(Arc::new(declaration))
    }

    pub fn register_shared(&mut self, declaration: Arc<dyn ANodeDeclaration>) -> Result<(), RegistryError> {
        let id = declaration.type_id();
        if self.declarations.contains_key(&id) {
            return Err(RegistryError::DuplicateANode(id));
        }
        self.declarations.insert(id, declaration);
        Ok(())
    }

    #[must_use]
    pub fn get(&self, id: &ANodeTypeId) -> Option<&Arc<dyn ANodeDeclaration>> {
        self.declarations.get(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Arc<dyn ANodeDeclaration>> {
        self.declarations.values()
    }

    pub fn declarations_with_role(&self, role: SurfaceItemKind) -> impl Iterator<Item = &Arc<dyn ANodeDeclaration>> {
        self.iter().filter(move |declaration| declaration.supports_role(role))
    }
}

fn primitive_descriptors() -> Vec<ValueTypeDescriptor> {
    vec![
        ValueTypeDescriptor::new(ValueTypeId::new("unit"), "Unit", ValueStorageKind::Unit, || {
            RuntimeValue::Unit
        })
        .with_conversion(ValueTypeId::new("bool"), ConversionKind::NonLossy)
        .with_conversion(ValueTypeId::new("int"), ConversionKind::NonLossy)
        .with_conversion(ValueTypeId::new("float"), ConversionKind::NonLossy)
        .with_conversion(ValueTypeId::new("string"), ConversionKind::NonLossy)
        .with_conversion(ValueTypeId::new("vec2"), ConversionKind::ScalarBroadcast)
        .with_conversion(ValueTypeId::new("vec3"), ConversionKind::ScalarBroadcast)
        .with_conversion(ValueTypeId::new("color"), ConversionKind::ScalarBroadcast)
        .with_conversion(ValueTypeId::new("duration"), ConversionKind::NonLossy),
        ValueTypeDescriptor::new(
            ValueTypeId::new("bool"),
            "Boolean",
            ValueStorageKind::InlineBool,
            || RuntimeValue::Bool(false),
        )
        .with_conversion(ValueTypeId::new("unit"), ConversionKind::Lossy)
        .with_conversion(ValueTypeId::new("int"), ConversionKind::NonLossy)
        .with_conversion(ValueTypeId::new("float"), ConversionKind::NonLossy)
        .with_conversion(ValueTypeId::new("string"), ConversionKind::NonLossy)
        .with_conversion(ValueTypeId::new("vec2"), ConversionKind::ScalarBroadcast)
        .with_conversion(ValueTypeId::new("vec3"), ConversionKind::ScalarBroadcast)
        .with_conversion(ValueTypeId::new("color"), ConversionKind::ScalarBroadcast)
        .with_conversion(ValueTypeId::new("duration"), ConversionKind::Lossy),
        ValueTypeDescriptor::new(
            ValueTypeId::new("trigger"),
            "Trigger",
            ValueStorageKind::Trigger,
            || RuntimeValue::Trigger(TriggerValue::default()),
        )
        .with_conversion(ValueTypeId::new("unit"), ConversionKind::Lossy)
        .with_conversion(ValueTypeId::new("bool"), ConversionKind::NonLossy)
        .with_conversion(ValueTypeId::new("int"), ConversionKind::NonLossy)
        .with_conversion(ValueTypeId::new("float"), ConversionKind::NonLossy)
        .with_conversion(ValueTypeId::new("string"), ConversionKind::NonLossy)
        .with_conversion(ValueTypeId::new("vec2"), ConversionKind::ScalarBroadcast)
        .with_conversion(ValueTypeId::new("vec3"), ConversionKind::ScalarBroadcast)
        .with_conversion(ValueTypeId::new("color"), ConversionKind::ScalarBroadcast)
        .with_conversion(ValueTypeId::new("duration"), ConversionKind::Lossy),
        ValueTypeDescriptor::new(ValueTypeId::new("int"), "Integer", ValueStorageKind::InlineI64, || {
            RuntimeValue::Int(0)
        })
        .with_conversion(ValueTypeId::new("unit"), ConversionKind::Lossy)
        .with_conversion(ValueTypeId::new("bool"), ConversionKind::Lossy)
        .with_conversion(ValueTypeId::new("float"), ConversionKind::NonLossy)
        .with_conversion(ValueTypeId::new("string"), ConversionKind::NonLossy)
        .with_conversion(ValueTypeId::new("vec2"), ConversionKind::ScalarBroadcast)
        .with_conversion(ValueTypeId::new("vec3"), ConversionKind::ScalarBroadcast)
        .with_conversion(ValueTypeId::new("color"), ConversionKind::ScalarBroadcast)
        .with_conversion(ValueTypeId::new("duration"), ConversionKind::Lossy),
        ValueTypeDescriptor::new(ValueTypeId::new("float"), "Float", ValueStorageKind::InlineF64, || {
            RuntimeValue::Float(0.0)
        })
        .with_conversion(ValueTypeId::new("unit"), ConversionKind::Lossy)
        .with_conversion(ValueTypeId::new("bool"), ConversionKind::Lossy)
        .with_conversion(ValueTypeId::new("int"), ConversionKind::Lossy)
        .with_conversion(ValueTypeId::new("string"), ConversionKind::NonLossy)
        .with_conversion(ValueTypeId::new("vec2"), ConversionKind::ScalarBroadcast)
        .with_conversion(ValueTypeId::new("vec3"), ConversionKind::ScalarBroadcast)
        .with_conversion(ValueTypeId::new("color"), ConversionKind::ScalarBroadcast)
        .with_conversion(ValueTypeId::new("duration"), ConversionKind::Lossy),
        ValueTypeDescriptor::new(
            ValueTypeId::new("string"),
            "String",
            ValueStorageKind::SharedString,
            || RuntimeValue::String(Arc::from("")),
        )
        .with_conversion(ValueTypeId::new("unit"), ConversionKind::Lossy)
        .with_conversion(ValueTypeId::new("bool"), ConversionKind::Lossy)
        .with_conversion(ValueTypeId::new("int"), ConversionKind::Lossy)
        .with_conversion(ValueTypeId::new("float"), ConversionKind::Lossy)
        .with_conversion(ValueTypeId::new("vec2"), ConversionKind::Lossy)
        .with_conversion(ValueTypeId::new("vec3"), ConversionKind::Lossy)
        .with_conversion(ValueTypeId::new("color"), ConversionKind::Lossy)
        .with_conversion(ValueTypeId::new("duration"), ConversionKind::Lossy),
        ValueTypeDescriptor::new(
            ValueTypeId::new("vec2"),
            "Vector 2",
            ValueStorageKind::InlineVec2,
            || RuntimeValue::Vec2([0.0; 2]),
        )
        .with_conversion(ValueTypeId::new("unit"), ConversionKind::Lossy)
        .with_conversion(ValueTypeId::new("bool"), ConversionKind::Lossy)
        .with_conversion(ValueTypeId::new("int"), ConversionKind::Lossy)
        .with_conversion(ValueTypeId::new("float"), ConversionKind::Lossy)
        .with_conversion(ValueTypeId::new("string"), ConversionKind::NonLossy)
        .with_conversion(ValueTypeId::new("vec3"), ConversionKind::NonLossy)
        .with_conversion(ValueTypeId::new("color"), ConversionKind::NonLossy)
        .with_conversion(ValueTypeId::new("duration"), ConversionKind::Lossy),
        ValueTypeDescriptor::new(
            ValueTypeId::new("vec3"),
            "Vector 3",
            ValueStorageKind::InlineVec3,
            || RuntimeValue::Vec3([0.0; 3]),
        )
        .with_conversion(ValueTypeId::new("unit"), ConversionKind::Lossy)
        .with_conversion(ValueTypeId::new("bool"), ConversionKind::Lossy)
        .with_conversion(ValueTypeId::new("int"), ConversionKind::Lossy)
        .with_conversion(ValueTypeId::new("float"), ConversionKind::Lossy)
        .with_conversion(ValueTypeId::new("string"), ConversionKind::NonLossy)
        .with_conversion(ValueTypeId::new("vec2"), ConversionKind::Lossy)
        .with_conversion(ValueTypeId::new("color"), ConversionKind::NonLossy)
        .with_conversion(ValueTypeId::new("duration"), ConversionKind::Lossy),
        ValueTypeDescriptor::new(
            ValueTypeId::new("color"),
            "Color",
            ValueStorageKind::InlineColor,
            || RuntimeValue::Color(ColorValue::BLACK),
        )
        .with_conversion(ValueTypeId::new("unit"), ConversionKind::Lossy)
        .with_conversion(ValueTypeId::new("bool"), ConversionKind::Lossy)
        .with_conversion(ValueTypeId::new("int"), ConversionKind::Lossy)
        .with_conversion(ValueTypeId::new("float"), ConversionKind::Lossy)
        .with_conversion(ValueTypeId::new("string"), ConversionKind::NonLossy)
        .with_conversion(ValueTypeId::new("vec2"), ConversionKind::Lossy)
        .with_conversion(ValueTypeId::new("vec3"), ConversionKind::Lossy)
        .with_conversion(ValueTypeId::new("duration"), ConversionKind::Lossy),
        ValueTypeDescriptor::new(
            ValueTypeId::new("duration"),
            "Duration",
            ValueStorageKind::Duration,
            || RuntimeValue::Duration(std::time::Duration::ZERO),
        )
        .with_conversion(ValueTypeId::new("unit"), ConversionKind::Lossy)
        .with_conversion(ValueTypeId::new("bool"), ConversionKind::Lossy)
        .with_conversion(ValueTypeId::new("int"), ConversionKind::Lossy)
        .with_conversion(ValueTypeId::new("float"), ConversionKind::NonLossy)
        .with_conversion(ValueTypeId::new("string"), ConversionKind::NonLossy)
        .with_conversion(ValueTypeId::new("vec2"), ConversionKind::ScalarBroadcast)
        .with_conversion(ValueTypeId::new("vec3"), ConversionKind::ScalarBroadcast)
        .with_conversion(ValueTypeId::new("color"), ConversionKind::ScalarBroadcast),
        ValueTypeDescriptor::new(
            ValueTypeId::new("value_array"),
            "Value Array",
            ValueStorageKind::Array,
            || RuntimeValue::Array(Vec::new()),
        )
        .with_conversion(ValueTypeId::new("string"), ConversionKind::Lossy),
    ]
}
