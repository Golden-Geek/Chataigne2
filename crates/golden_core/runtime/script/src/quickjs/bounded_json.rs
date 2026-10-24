use serde::de::{DeserializeSeed, Error, MapAccess, SeqAccess, Visitor};
use serde_json::Value;
use std::fmt;

// Includes the root array, so 4096 scalar arguments remain legal. Limits apply
// during parsing, before serde can allocate a wide/deep intermediate tree.
const MAX_NODES: usize = 4097;
const MAX_DEPTH: usize = 32;

pub(super) fn parse(source: &str) -> Result<Value, serde_json::Error> {
    parse_with_limits(source, MAX_NODES, MAX_DEPTH)
}

pub(super) fn parse_with_limits(
    source: &str,
    maximum_nodes: usize,
    maximum_depth: usize,
) -> Result<Value, serde_json::Error> {
    let mut nodes = 0;
    let mut deserializer = serde_json::Deserializer::from_str(source);
    let value = Seed {
        nodes: &mut nodes,
        depth: 0,
        maximum_nodes,
        maximum_depth,
    }
    .deserialize(&mut deserializer)?;
    deserializer.end()?;
    Ok(value)
}

struct Seed<'a> {
    nodes: &'a mut usize,
    depth: usize,
    maximum_nodes: usize,
    maximum_depth: usize,
}

impl<'de> DeserializeSeed<'de> for Seed<'_> {
    type Value = Value;
    fn deserialize<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        *self.nodes += 1;
        if *self.nodes > self.maximum_nodes || self.depth > self.maximum_depth {
            return Err(D::Error::custom("script method JSON exceeds its item/depth limit"));
        }
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Seed<'_> {
    type Value = Value;
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("bounded script method JSON")
    }
    fn visit_unit<E: Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_bool<E: Error>(self, value: bool) -> Result<Value, E> {
        Ok(value.into())
    }
    fn visit_i64<E: Error>(self, value: i64) -> Result<Value, E> {
        Ok(value.into())
    }
    fn visit_u64<E: Error>(self, value: u64) -> Result<Value, E> {
        Ok(value.into())
    }
    fn visit_f64<E: Error>(self, value: f64) -> Result<Value, E> {
        serde_json::Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("nonfinite number"))
    }
    fn visit_str<E: Error>(self, value: &str) -> Result<Value, E> {
        Ok(Value::String(value.to_owned()))
    }
    fn visit_string<E: Error>(self, value: String) -> Result<Value, E> {
        Ok(Value::String(value))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Value, A::Error> {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element_seed(Seed {
            nodes: self.nodes,
            depth: self.depth + 1,
            maximum_nodes: self.maximum_nodes,
            maximum_depth: self.maximum_depth,
        })? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut values = serde_json::Map::new();
        while let Some(key) = map.next_key::<String>()? {
            let value = map.next_value_seed(Seed {
                nodes: self.nodes,
                depth: self.depth + 1,
                maximum_nodes: self.maximum_nodes,
                maximum_depth: self.maximum_depth,
            })?;
            values.insert(key, value);
        }
        Ok(Value::Object(values))
    }
}

#[cfg(test)]
mod tests;
