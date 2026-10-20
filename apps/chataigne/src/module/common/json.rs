use serde::de::{DeserializeSeed, Error, IgnoredAny, MapAccess, SeqAccess, Visitor};
use std::fmt;

/// Reject expansion before constructing a wide/deep value tree. The first pass
/// validates through serde without retaining values; only bounded input is built.
pub(crate) fn parse(source: &str, bytes: usize, nodes: usize, depth: usize) -> Result<serde_json::Value, String> {
    if source.len() > bytes {
        return Err("JSON source exceeds its byte limit".into());
    }
    let mut budget = Budget {
        remaining: nodes,
        depth,
    };
    let mut deserializer = serde_json::Deserializer::from_str(source);
    Seed {
        budget: &mut budget,
        level: 0,
    }
    .deserialize(&mut deserializer)
    .map_err(|error| error.to_string())?;
    deserializer.end().map_err(|error| error.to_string())?;
    serde_json::from_str(source).map_err(|error| error.to_string())
}
struct Budget {
    remaining: usize,
    depth: usize,
}
struct Seed<'a> {
    budget: &'a mut Budget,
    level: usize,
}
impl<'de> DeserializeSeed<'de> for Seed<'_> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        if self.budget.remaining == 0 || self.level > self.budget.depth {
            return Err(D::Error::custom("JSON exceeds its item/depth limit"));
        }
        self.budget.remaining -= 1;
        deserializer.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for Seed<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bounded JSON")
    }
    fn visit_unit<E: Error>(self) -> Result<(), E> {
        Ok(())
    }
    fn visit_bool<E: Error>(self, _: bool) -> Result<(), E> {
        Ok(())
    }
    fn visit_i64<E: Error>(self, _: i64) -> Result<(), E> {
        Ok(())
    }
    fn visit_u64<E: Error>(self, _: u64) -> Result<(), E> {
        Ok(())
    }
    fn visit_f64<E: Error>(self, _: f64) -> Result<(), E> {
        Ok(())
    }
    fn visit_str<E: Error>(self, _: &str) -> Result<(), E> {
        Ok(())
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        while seq
            .next_element_seed(Seed {
                budget: self.budget,
                level: self.level + 1,
            })?
            .is_some()
        {}
        Ok(())
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        while map.next_key::<IgnoredAny>()?.is_some() {
            map.next_value_seed(Seed {
                budget: self.budget,
                level: self.level + 1,
            })?;
        }
        Ok(())
    }
}

/// A conservative allocation budget checked before any JSON clone or typed
/// decode. Arrays include spare capacity; maps include per-entry tree overhead.
pub(crate) fn retained_weight(
    value: &serde_json::Value,
    bytes: usize,
    items: usize,
    depth: usize,
) -> Result<usize, String> {
    use serde_json::Value;
    let mut pending = vec![(value, 0_usize)];
    let mut visited = 0_usize;
    let mut weight = std::mem::size_of::<Value>();
    while let Some((value, level)) = pending.pop() {
        visited += 1;
        if visited > items || level > depth {
            return Err("JSON exceeds its item/depth admission limit".into());
        }
        match value {
            Value::String(text) => weight = weight.saturating_add(text.capacity()),
            Value::Array(values) => {
                weight = weight.saturating_add(values.capacity().saturating_mul(std::mem::size_of::<Value>()));
                if weight > bytes || values.len().saturating_add(visited).saturating_add(pending.len()) > items {
                    return Err("JSON exceeds its retained-byte/item admission limit".into());
                }
                pending.extend(values.iter().map(|value| (value, level + 1)));
            }
            Value::Object(values) => {
                weight = weight.saturating_add(values.len().saturating_mul(256));
                if weight > bytes || values.len().saturating_add(visited).saturating_add(pending.len()) > items {
                    return Err("JSON exceeds its retained-byte/item admission limit".into());
                }
                for (key, value) in values {
                    weight = weight.saturating_add(key.capacity());
                    pending.push((value, level + 1));
                }
            }
            _ => {}
        }
        if weight > bytes {
            return Err("JSON exceeds its retained-byte admission limit".into());
        }
    }
    Ok(weight)
}

#[cfg(test)]
mod tests;
