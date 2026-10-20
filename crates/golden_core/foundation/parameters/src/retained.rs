use crate::{NodeReference, ParamValue, ParameterEnumOption};

fn sum(values: impl IntoIterator<Item = usize>) -> usize {
    values.into_iter().fold(0usize, usize::saturating_add)
}

fn strings(value: &Vec<String>) -> usize {
    sum([
        value.capacity().saturating_mul(size_of::<String>()),
        sum(value.iter().map(String::capacity)),
    ])
}

fn inline_only<T: Copy>(_: &T) {}

impl NodeReference {
    /// Conservative owned allocation charge, including cached hints and spare
    /// path capacity. Semantic reference equality deliberately ignores hints.
    pub fn retained_heap_bytes(&self) -> usize {
        let Self {
            uuid,
            projection,
            cached_name,
            cached_id,
            relative_path_from_root,
        } = self;
        inline_only(uuid);
        inline_only(projection);
        inline_only(cached_id);
        sum([
            cached_name.as_ref().map_or(0, String::capacity),
            strings(relative_path_from_root),
        ])
    }
}

impl ParamValue {
    /// Conservative owned allocation charge, excluding this inline value.
    pub fn retained_heap_bytes(&self) -> usize {
        match self {
            Self::Str(value) | Self::File(value) | Self::Enum(value) => value.capacity(),
            Self::Reference(value) => value.retained_heap_bytes(),
            Self::Trigger()
            | Self::Int(_)
            | Self::Float(_)
            | Self::Bool(_)
            | Self::CssValue(_)
            | Self::Vec2(..)
            | Self::Vec3(..)
            | Self::Color(..) => 0,
        }
    }
}

impl ParameterEnumOption {
    /// Conservative owned allocation charge, excluding this inline option.
    /// Intended for producer-side domain admission before freezing storage.
    pub fn retained_heap_bytes(&self) -> usize {
        let Self {
            variant_id,
            value,
            label,
            tags,
            ordering,
        } = self;
        inline_only(ordering);
        sum([
            variant_id.capacity(),
            value.retained_heap_bytes(),
            label.capacity(),
            strings(tags),
        ])
    }
}
