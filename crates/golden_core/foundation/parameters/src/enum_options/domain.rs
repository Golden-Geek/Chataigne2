use serde::Serialize;

use crate::{ParamValue, ParameterEnumOption};

/// Frozen enum entries, membership indexes and cached allocation charge.
///
/// Construct on the producing/IO boundary. Clones and exported owners share
/// this whole object, so native lifetime admission also protects its indexes.
#[derive(Serialize)]
#[serde(transparent)]
pub struct ParameterEnumDomain {
    options: Vec<ParameterEnumOption>,
    #[serde(skip)]
    by_variant: Box<[usize]>,
    #[serde(skip)]
    by_value: Option<Box<[usize]>>,
    #[serde(skip)]
    retained_bytes: usize,
}

impl ParameterEnumDomain {
    /// Freezes authored order and spare capacity, building sorted slot indexes
    /// without duplicating option strings or values. Canonical enum values share
    /// the variant index; other values need a second index. Sorting is O(n log n).
    pub fn new(options: Vec<ParameterEnumOption>) -> Self {
        let canonical = canonical_values(&options);
        let retained_bytes = storage_bytes(&options, canonical);
        let mut by_variant: Vec<_> = (0..options.len()).collect();
        by_variant.sort_unstable_by(|left, right| options[*left].variant_id.cmp(&options[*right].variant_id));
        let by_value = (!canonical).then(|| {
            let mut slots: Vec<_> = (0..options.len()).collect();
            slots.sort_unstable_by(|left, right| {
                super::ordering::compare(&options[*left].value, &options[*right].value)
            });
            slots.into_boxed_slice()
        });
        Self {
            options,
            by_variant: by_variant.into_boxed_slice(),
            by_value,
            retained_bytes,
        }
    }

    /// Producer-side allocation charge before construction. Includes the final
    /// Arc header, option spare capacity, boxed indexes and all nested storage.
    /// This scans entries; use the cached `retained_bytes` after freezing.
    pub fn retained_bytes_for(options: &Vec<ParameterEnumOption>) -> usize {
        storage_bytes(options, canonical_values(options))
    }

    /// Constant-time conservative allocation charge, including shared storage
    /// in full. This is an admission reservation, not allocator/RSS telemetry.
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }

    /// Borrowed options in their authored order.
    pub fn as_slice(&self) -> &[ParameterEnumOption] {
        &self.options
    }

    /// Option-vector capacity, including spare slots.
    pub fn capacity(&self) -> usize {
        self.options.capacity()
    }

    pub(super) fn contains_variant(&self, variant: &str) -> bool {
        self.by_variant
            .binary_search_by(|index| self.options[*index].variant_id.as_str().cmp(variant))
            .is_ok()
    }

    pub(super) fn contains_value(&self, value: &ParamValue) -> bool {
        let Some(slots) = &self.by_value else {
            return matches!(value, ParamValue::Enum(variant) if self.contains_variant(variant));
        };
        slots
            .binary_search_by(|index| super::ordering::compare(&self.options[*index].value, value))
            .is_ok_and(|index| self.options[slots[index]].value == *value)
    }
}

fn canonical_values(options: &[ParameterEnumOption]) -> bool {
    options
        .iter()
        .all(|option| matches!(&option.value, ParamValue::Enum(value) if value == &option.variant_id))
}

fn storage_bytes(options: &Vec<ParameterEnumOption>, canonical: bool) -> usize {
    let index_count = if canonical { 1 } else { 2 };
    [
        size_of::<ParameterEnumDomain>(),
        2 * size_of::<usize>(),
        options.capacity().saturating_mul(size_of::<ParameterEnumOption>()),
        options.len().saturating_mul(index_count * size_of::<usize>()),
    ]
    .into_iter()
    .chain(options.iter().map(ParameterEnumOption::retained_heap_bytes))
    .fold(0usize, usize::saturating_add)
}

impl std::fmt::Debug for ParameterEnumDomain {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Index contents are derived and need not amplify diagnostic output.
        formatter
            .debug_struct("ParameterEnumDomain")
            .field("options", &self.options)
            .finish_non_exhaustive()
    }
}
