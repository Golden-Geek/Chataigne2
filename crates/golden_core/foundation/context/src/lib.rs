use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use golden_model::NodeId;
use golden_parameters::{ParamValue, ParamValueProjection};

const MULTIPLEX_CONTEXT_LINK_PREFIX: &str = "@multiplex";

/// Runtime target encoded in a context-link symbol for one multiplex lane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MultiplexContextLinkTarget {
    /// Zero- or one-based position of the lane item on an axis.
    Index {
        /// Stable multiplex axis id.
        axis_id: String,
        /// Whether the index starts at zero instead of one.
        zero_based: bool,
    },
    /// One list value on an axis.
    List {
        /// Stable multiplex axis id.
        axis_id: String,
        /// List symbol within that multiplex.
        symbol: String,
    },
}

/// Optional multiplex selector used by smart string tokens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MultiplexTokenSelector {
    /// First available multiplex.
    First,
    /// One-based ordinal among available multiplexes.
    Ordinal(usize),
    /// Multiplex display name.
    Name(String),
}

/// Smart string token resolved by a multiplex-aware lane runtime.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MultiplexTemplateToken {
    /// Current lane index.
    Index {
        /// Whether the index starts at zero instead of one.
        zero_based: bool,
        /// Multiplex selected by default, ordinal, or name.
        multiplex: MultiplexTokenSelector,
    },
    /// Current entry from a named list.
    List {
        /// Multiplex selected by default, ordinal, or name.
        multiplex: MultiplexTokenSelector,
        /// List symbol within the selected multiplex.
        list: String,
    },
}

/// Builds the opaque context-link symbol for a multiplex lane index.
pub fn multiplex_index_context_link_symbol(axis_id: &str, zero_based: bool) -> String {
    let basis = if zero_based { "index0" } else { "index" };
    format!("{MULTIPLEX_CONTEXT_LINK_PREFIX}:{axis_id}:{basis}")
}

/// Builds the opaque context-link symbol for one list on a multiplex axis.
pub fn multiplex_list_context_link_symbol(axis_id: &str, symbol: &str) -> String {
    format!("{MULTIPLEX_CONTEXT_LINK_PREFIX}:{axis_id}:list:{symbol}")
}

/// Parses an opaque multiplex context-link symbol.
pub fn parse_multiplex_context_link_symbol(symbol: &str) -> Option<MultiplexContextLinkTarget> {
    let suffix = symbol
        .trim()
        .strip_prefix(MULTIPLEX_CONTEXT_LINK_PREFIX)?
        .strip_prefix(':')?;
    let (axis_id, target) = suffix.split_once(':')?;
    if axis_id.is_empty() {
        return None;
    }
    match target {
        "index" => Some(MultiplexContextLinkTarget::Index {
            axis_id: axis_id.to_owned(),
            zero_based: false,
        }),
        "index0" => Some(MultiplexContextLinkTarget::Index {
            axis_id: axis_id.to_owned(),
            zero_based: true,
        }),
        _ => target
            .strip_prefix("list:")
            .filter(|list| !list.is_empty())
            .map(|list| MultiplexContextLinkTarget::List {
                axis_id: axis_id.to_owned(),
                symbol: list.to_owned(),
            }),
    }
}

/// Parses `{index}`, `{index0}`, and `{list:...}` token contents.
pub fn parse_multiplex_template_token(token: &str) -> Option<MultiplexTemplateToken> {
    let token = token.trim();
    if token == "index" || token == "index0" {
        return Some(MultiplexTemplateToken::Index {
            zero_based: token == "index0",
            multiplex: MultiplexTokenSelector::First,
        });
    }
    if let Some(selector) = token.strip_prefix("index:") {
        return parse_multiplex_token_selector(selector).map(|multiplex| MultiplexTemplateToken::Index {
            zero_based: false,
            multiplex,
        });
    }
    if let Some(selector) = token.strip_prefix("index0:") {
        return parse_multiplex_token_selector(selector).map(|multiplex| MultiplexTemplateToken::Index {
            zero_based: true,
            multiplex,
        });
    }
    let list = token.strip_prefix("list:")?.trim();
    if list.is_empty() {
        return None;
    }
    let (multiplex, list) = match list.split_once(':') {
        Some((multiplex, list)) if !multiplex.trim().is_empty() && !list.trim().is_empty() => {
            (parse_multiplex_token_selector(multiplex)?, list.trim().to_owned())
        }
        _ => (MultiplexTokenSelector::First, list.to_owned()),
    };
    Some(MultiplexTemplateToken::List { multiplex, list })
}

fn parse_multiplex_token_selector(selector: &str) -> Option<MultiplexTokenSelector> {
    let selector = selector.trim();
    if selector.is_empty() {
        return None;
    }
    Some(match selector.parse::<usize>() {
        Ok(ordinal) if ordinal > 0 => MultiplexTokenSelector::Ordinal(ordinal),
        _ => MultiplexTokenSelector::Name(selector.to_owned()),
    })
}

/// Typed value family used by `UserContext` entries and lookups.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum UserContextValueType {
    /// Trigger pulse.
    Trigger,
    /// Signed integer.
    Int,
    /// Floating-point scalar.
    Float,
    /// UTF-8 string.
    Str,
    /// File-path string.
    File,
    /// Enum variant id.
    Enum,
    /// Boolean.
    Bool,
    /// CSS scalar value.
    #[serde(rename = "css_value")]
    CssValue,
    /// 2D vector.
    Vec2,
    /// 3D vector.
    Vec3,
    /// RGBA color.
    Color,
    /// Node reference.
    Reference,
}

impl UserContextValueType {
    /// Infers the value type from a parameter value.
    pub fn from_param_value(value: &ParamValue) -> Self {
        match value {
            ParamValue::Trigger() => Self::Trigger,
            ParamValue::Int(_) => Self::Int,
            ParamValue::Float(_) => Self::Float,
            ParamValue::Str(_) => Self::Str,
            ParamValue::File(_) => Self::File,
            ParamValue::Enum(_) => Self::Enum,
            ParamValue::Bool(_) => Self::Bool,
            ParamValue::CssValue(_) => Self::CssValue,
            ParamValue::Vec2(_, _) => Self::Vec2,
            ParamValue::Vec3(_, _, _) => Self::Vec3,
            ParamValue::Color(_, _, _, _) => Self::Color,
            ParamValue::Reference(_) => Self::Reference,
        }
    }

    /// Infers the value type from a built-in parameter node type id.
    pub fn from_parameter_node_type(node_type: &str) -> Option<Self> {
        match node_type {
            "trigger" => Some(Self::Trigger),
            "int" => Some(Self::Int),
            "float" => Some(Self::Float),
            "str" => Some(Self::Str),
            "file" => Some(Self::File),
            "enum" => Some(Self::Enum),
            "bool" => Some(Self::Bool),
            "css_value" => Some(Self::CssValue),
            "vec2" => Some(Self::Vec2),
            "vec3" => Some(Self::Vec3),
            "color" => Some(Self::Color),
            "reference" => Some(Self::Reference),
            _ => None,
        }
    }
}

/// Context entry kind.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum UserContextEntryKind {
    /// Single scalar parameter entry.
    #[default]
    Scalar,
    /// Multiplex list entry resolved per runtime lane.
    MultiplexList,
}

/// One direct parameter entry inside a multiplex list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UserContextMultiplexListEntry {
    /// Entry parameter node.
    pub param: NodeId,
    /// Stable lane item id derived from the direct entry parameter UUID.
    pub item_id: String,
    /// Zero-based display/order index.
    pub index: u32,
}

/// Metadata for one multiplex list context entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UserContextMultiplexList {
    /// Multiplex node hosting the list.
    pub multiplex: NodeId,
    /// Typed list node.
    pub list: NodeId,
    /// Stable axis id derived from the multiplex node UUID.
    pub axis_id: String,
    /// Opaque link target for the one-based lane index.
    pub index_link_symbol: String,
    /// Opaque link target for the zero-based lane index.
    pub index0_link_symbol: String,
    /// Opaque link target for this list.
    pub list_link_symbol: String,
    /// Entry value type.
    pub value_type: UserContextValueType,
    /// Direct parameter entries in display order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<UserContextMultiplexListEntry>,
}

/// One owned `UserContext` entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UserContextEntry {
    /// Symbol name used in lexical lookups.
    pub symbol: String,
    /// Backing parameter node for scalar entries, or typed list node for multiplex lists.
    pub param: NodeId,
    /// Entry value type.
    pub value_type: UserContextValueType,
    /// Entry kind.
    #[serde(default)]
    pub kind: UserContextEntryKind,
    /// Multiplex metadata for list entries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multiplex: Option<UserContextMultiplexList>,
}

/// One `UserContext` scope owned by a node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UserContextScope {
    /// Scope owner node id.
    pub owner: NodeId,
    /// Monotonic generation incremented on schema edits.
    pub generation: u64,
    /// Entries keyed by symbol.
    pub entries: HashMap<String, UserContextEntry>,
}

impl UserContextScope {
    /// Creates an empty scope for `owner`.
    pub fn new(owner: NodeId) -> Self {
        Self {
            owner,
            generation: 0,
            entries: HashMap::new(),
        }
    }
}

/// Successful lexical resolution result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UserContextResolution {
    /// Requested symbol.
    pub symbol: String,
    /// Resolved entry parameter node.
    pub entry_param: NodeId,
    /// Resolved value type.
    pub value_type: UserContextValueType,
    /// Entry kind.
    #[serde(default)]
    pub kind: UserContextEntryKind,
    /// Multiplex metadata for list entries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multiplex: Option<UserContextMultiplexList>,
    /// Scope owner node id.
    pub scope_owner: NodeId,
    /// Ancestor distance from consumer (`0` means same node).
    pub lexical_depth: u32,
}

/// Type mismatch details for lexical resolution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UserContextTypeMismatch {
    /// Requested symbol.
    pub symbol: String,
    /// Expected value type.
    pub expected: UserContextValueType,
    /// Found value type.
    pub found: UserContextValueType,
    /// Entry parameter node.
    pub entry_param: NodeId,
    /// Scope owner node id.
    pub scope_owner: NodeId,
    /// Ancestor distance from consumer (`0` means same node).
    pub lexical_depth: u32,
}

/// Lexical lookup result for one `UserContext` symbol.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum UserContextLookup {
    /// Symbol resolved successfully.
    Resolved(UserContextResolution),
    /// Nearest symbol exists but has incompatible type.
    TypeMismatch(UserContextTypeMismatch),
    /// Symbol is not found in visible lexical scopes.
    Missing {
        /// Requested symbol.
        symbol: String,
    },
}

/// UI/query candidate entry returned for one consumer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UserContextCandidate {
    /// Entry symbol.
    pub symbol: String,
    /// Entry value type.
    pub value_type: UserContextValueType,
    /// Entry kind.
    #[serde(default)]
    pub kind: UserContextEntryKind,
    /// Multiplex metadata for list entries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multiplex: Option<UserContextMultiplexList>,
    /// Scope owner node id.
    pub scope_owner: NodeId,
    /// Ancestor distance from consumer (`0` means same node).
    pub lexical_depth: u32,
    /// Backing parameter node id.
    pub entry_param: NodeId,
    /// Whether the candidate is compatible with the expected type.
    pub compatible: bool,
    /// Whether the source can be coerced without selecting a projection.
    #[serde(default)]
    pub directly_compatible: bool,
    /// Whether this candidate's multiplex indexes can be linked to the target parameter.
    #[serde(default)]
    pub multiplex_index_compatible: bool,
    /// Whether this candidate is shadowed by a nearer scope with the same symbol.
    pub shadowed: bool,
    /// Optional projections that can make this candidate compatible.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub projections: Vec<ParamValueProjection>,
}

/// UI payload returned by per-parameter context-candidate queries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiUserContextCandidatesDto {
    /// Target parameter node.
    pub param: NodeId,
    /// Expected value type for compatibility filtering.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected: Option<UserContextValueType>,
    /// Visible lexical candidates.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<UserContextCandidate>,
}

/// UI payload for one context entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiUserContextEntryDto {
    /// Symbol name.
    pub symbol: String,
    /// Backing parameter node id.
    pub param: NodeId,
    /// Entry value type.
    pub value_type: UserContextValueType,
    /// Entry kind.
    #[serde(default)]
    pub kind: UserContextEntryKind,
    /// Multiplex metadata for list entries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multiplex: Option<UserContextMultiplexList>,
}

/// UI payload for one scope owned by one node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiUserContextScopeDto {
    /// Scope owner node id.
    pub owner: NodeId,
    /// Scope generation.
    pub generation: u64,
    /// Scope entries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<UiUserContextEntryDto>,
}

/// UI payload for all user context scopes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiUserContextsDto {
    /// Registered scopes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<UiUserContextScopeDto>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct UserContextCacheKey {
    consumer: NodeId,
    symbol: String,
    expected: Option<UserContextValueType>,
}

#[derive(Clone, Debug)]
struct UserContextCacheValue {
    lookup: UserContextLookup,
    graph_generation: u64,
    schema_generation: u64,
}

/// Runtime registry for owned lexical `UserContext` scopes.
#[derive(Default)]
pub struct UserContextRegistry {
    scopes_by_owner: HashMap<NodeId, UserContextScope>,
    cache: HashMap<UserContextCacheKey, UserContextCacheValue>,
    graph_generation: u64,
    schema_generation: u64,
}

impl UserContextRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns all registered scopes keyed by owner id.
    pub fn scopes(&self) -> &HashMap<NodeId, UserContextScope> {
        &self.scopes_by_owner
    }

    /// Returns one scope for `owner`.
    pub fn scope(&self, owner: NodeId) -> Option<&UserContextScope> {
        self.scopes_by_owner.get(&owner)
    }

    /// Ensures a scope exists for `owner`.
    ///
    /// Returns `true` when a new scope was created.
    pub fn ensure_scope(&mut self, owner: NodeId) -> bool {
        if self.scopes_by_owner.contains_key(&owner) {
            return false;
        }
        self.scopes_by_owner.insert(owner, UserContextScope::new(owner));
        self.bump_schema_generation();
        true
    }

    /// Removes one scope by owner id.
    ///
    /// Returns `true` when one scope was removed.
    pub fn remove_scope(&mut self, owner: NodeId) -> bool {
        if self.scopes_by_owner.remove(&owner).is_none() {
            return false;
        }
        self.bump_schema_generation();
        true
    }

    /// Adds or replaces one entry in `owner` scope.
    ///
    /// Returns `true` when the scope schema changed.
    pub fn upsert_entry(
        &mut self,
        owner: NodeId,
        symbol: impl Into<String>,
        param: NodeId,
        value_type: UserContextValueType,
    ) -> Result<bool, String> {
        self.upsert_entry_kind(owner, symbol, param, value_type, UserContextEntryKind::Scalar, None)
    }

    /// Adds or replaces one multiplex list entry in `owner` scope.
    ///
    /// Returns `true` when the scope schema changed.
    pub fn upsert_multiplex_list_entry(
        &mut self,
        owner: NodeId,
        symbol: impl Into<String>,
        list: UserContextMultiplexList,
    ) -> Result<bool, String> {
        let value_type = list.value_type;
        let param = list.list;
        self.upsert_entry_kind(
            owner,
            symbol,
            param,
            value_type,
            UserContextEntryKind::MultiplexList,
            Some(list),
        )
    }

    /// Adds an additional multiplex list candidate whose display symbol is already present.
    ///
    /// The opaque list-link symbol is used only as the storage key, so lexical lookup by the
    /// plain symbol keeps resolving the first entry while UIs can still enumerate every list.
    pub fn upsert_additional_multiplex_list_entry(
        &mut self,
        owner: NodeId,
        symbol: impl Into<String>,
        list: UserContextMultiplexList,
    ) -> Result<bool, String> {
        let symbol = symbol.into().trim().to_string();
        let key = list.list_link_symbol.clone();
        let value_type = list.value_type;
        let param = list.list;
        self.upsert_entry_kind_with_key(
            owner,
            key,
            UserContextEntry {
                symbol,
                param,
                value_type,
                kind: UserContextEntryKind::MultiplexList,
                multiplex: Some(list),
            },
        )
    }

    fn upsert_entry_kind(
        &mut self,
        owner: NodeId,
        symbol: impl Into<String>,
        param: NodeId,
        value_type: UserContextValueType,
        kind: UserContextEntryKind,
        multiplex: Option<UserContextMultiplexList>,
    ) -> Result<bool, String> {
        let symbol = symbol.into().trim().to_string();
        self.upsert_entry_kind_with_key(
            owner,
            symbol.clone(),
            UserContextEntry {
                symbol,
                param,
                value_type,
                kind,
                multiplex,
            },
        )
    }

    fn upsert_entry_kind_with_key(
        &mut self,
        owner: NodeId,
        key: String,
        entry: UserContextEntry,
    ) -> Result<bool, String> {
        if entry.symbol.is_empty() {
            return Err("context symbol cannot be empty".to_string());
        }

        let Some(scope) = self.scopes_by_owner.get_mut(&owner) else {
            return Err(format!("context scope for owner {:?} does not exist", owner));
        };

        if scope.entries.get(&key) == Some(&entry) {
            return Ok(false);
        }

        scope.entries.insert(key, entry);
        scope.generation = scope.generation.saturating_add(1);
        self.bump_schema_generation();
        Ok(true)
    }

    /// Removes one entry from `owner` scope.
    ///
    /// Returns `true` when one entry was removed.
    pub fn remove_entry(&mut self, owner: NodeId, symbol: &str) -> bool {
        let symbol = symbol.trim();
        if symbol.is_empty() {
            return false;
        }

        let Some(scope) = self.scopes_by_owner.get_mut(&owner) else {
            return false;
        };

        if scope.entries.remove(symbol).is_none() {
            return false;
        }

        scope.generation = scope.generation.saturating_add(1);
        self.bump_schema_generation();
        true
    }

    /// Removes invalid scope/entry references.
    ///
    /// Returns `true` when registry content changed.
    pub fn prune<FOwner, FParam>(&mut self, mut owner_exists: FOwner, mut param_is_valid: FParam) -> bool
    where
        FOwner: FnMut(NodeId) -> bool,
        FParam: FnMut(NodeId) -> bool,
    {
        let mut changed = false;

        let owners_to_remove = self
            .scopes_by_owner
            .keys()
            .copied()
            .filter(|owner| !owner_exists(*owner))
            .collect::<Vec<_>>();
        for owner in owners_to_remove {
            self.scopes_by_owner.remove(&owner);
            changed = true;
        }

        for scope in self.scopes_by_owner.values_mut() {
            let before = scope.entries.len();
            scope.entries.retain(|_, entry| {
                param_is_valid(entry.param)
                    && entry
                        .multiplex
                        .as_ref()
                        .is_none_or(|list| list.entries.iter().all(|item| param_is_valid(item.param)))
            });
            if scope.entries.len() != before {
                scope.generation = scope.generation.saturating_add(1);
                changed = true;
            }
        }

        if changed {
            self.bump_schema_generation();
        }

        changed
    }

    /// Marks graph topology as changed and invalidates resolution cache.
    pub fn mark_graph_changed(&mut self) {
        self.graph_generation = self.graph_generation.saturating_add(1);
        self.cache.clear();
    }

    /// Resolves one symbol lexically from `consumer`.
    pub fn resolve_symbol<F>(
        &mut self,
        consumer: NodeId,
        symbol: &str,
        expected: Option<UserContextValueType>,
        mut parent_of: F,
    ) -> UserContextLookup
    where
        F: FnMut(NodeId) -> Option<NodeId>,
    {
        let symbol = symbol.trim().to_string();
        if symbol.is_empty() {
            return UserContextLookup::Missing { symbol: String::new() };
        }

        let key = UserContextCacheKey {
            consumer,
            symbol: symbol.clone(),
            expected,
        };
        if let Some(cached) = self.cache.get(&key)
            && cached.graph_generation == self.graph_generation
            && cached.schema_generation == self.schema_generation
        {
            return cached.lookup.clone();
        }

        let mut depth = 0u32;
        let mut cursor = Some(consumer);
        let mut lookup = UserContextLookup::Missing { symbol: symbol.clone() };

        while let Some(node_id) = cursor {
            if let Some(scope) = self.scopes_by_owner.get(&node_id)
                && let Some(entry) = scope.entries.get(&symbol)
            {
                lookup = match expected {
                    Some(expected) if expected != entry.value_type => {
                        UserContextLookup::TypeMismatch(UserContextTypeMismatch {
                            symbol: symbol.clone(),
                            expected,
                            found: entry.value_type,
                            entry_param: entry.param,
                            scope_owner: scope.owner,
                            lexical_depth: depth,
                        })
                    }
                    _ => UserContextLookup::Resolved(UserContextResolution {
                        symbol: symbol.clone(),
                        entry_param: entry.param,
                        value_type: entry.value_type,
                        kind: entry.kind,
                        multiplex: entry.multiplex.clone(),
                        scope_owner: scope.owner,
                        lexical_depth: depth,
                    }),
                };
                break;
            }

            cursor = parent_of(node_id);
            depth = depth.saturating_add(1);
        }

        self.cache.insert(
            key,
            UserContextCacheValue {
                lookup: lookup.clone(),
                graph_generation: self.graph_generation,
                schema_generation: self.schema_generation,
            },
        );

        lookup
    }

    /// Collects all visible candidate entries for one consumer.
    pub fn collect_candidates<F>(
        &self,
        consumer: NodeId,
        expected: Option<UserContextValueType>,
        mut parent_of: F,
    ) -> Vec<UserContextCandidate>
    where
        F: FnMut(NodeId) -> Option<NodeId>,
    {
        let mut out = Vec::<UserContextCandidate>::new();
        let mut seen_symbols = HashSet::<String>::new();
        let mut depth = 0u32;
        let mut cursor = Some(consumer);

        while let Some(node_id) = cursor {
            if let Some(scope) = self.scopes_by_owner.get(&node_id) {
                let mut entries = scope.entries.values().cloned().collect::<Vec<_>>();
                entries.sort_by(|left, right| left.symbol.cmp(&right.symbol));

                for entry in entries {
                    let shadowed = seen_symbols.contains(entry.symbol.as_str());
                    if !shadowed {
                        seen_symbols.insert(entry.symbol.clone());
                    }

                    let compatible = expected.is_none_or(|expected| expected == entry.value_type);
                    out.push(UserContextCandidate {
                        symbol: entry.symbol,
                        value_type: entry.value_type,
                        kind: entry.kind,
                        multiplex: entry.multiplex,
                        scope_owner: scope.owner,
                        lexical_depth: depth,
                        entry_param: entry.param,
                        compatible,
                        directly_compatible: compatible,
                        multiplex_index_compatible: false,
                        shadowed,
                        projections: Vec::new(),
                    });
                }
            }

            cursor = parent_of(node_id);
            depth = depth.saturating_add(1);
        }

        out.sort_by(|left, right| {
            left.lexical_depth
                .cmp(&right.lexical_depth)
                .then_with(|| left.symbol.cmp(&right.symbol))
                .then_with(|| left.entry_param.0.cmp(&right.entry_param.0))
        });
        out
    }

    fn bump_schema_generation(&mut self) {
        self.schema_generation = self.schema_generation.saturating_add(1);
        self.cache.clear();
    }
}

/// Stable identifier of one `DynamicContext` dimension.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DynamicContextDimId(pub String);

impl DynamicContextDimId {
    /// Creates a new id.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns id as string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Runtime frame for one active `DynamicContext` dimension.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DynamicContextFrame {
    /// Dimension identifier.
    pub dim: DynamicContextDimId,
    /// Current index in this dimension.
    pub index: u32,
    /// Declared extent.
    pub extent: u32,
    /// Optional stable instance id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance: Option<u64>,
}

/// Runtime frame-stack container for dynamic-context evaluation.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DynamicContext {
    /// Active nested frame stack (outer to inner).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dim_stack: Vec<DynamicContextFrame>,
}

impl DynamicContext {
    /// Creates an empty dynamic context stack.
    pub fn new() -> Self {
        Self::default()
    }

    /// Pushes one dimension frame.
    pub fn push_dim(&mut self, dim: DynamicContextDimId, index: u32, extent: u32, instance: Option<u64>) {
        self.dim_stack.push(DynamicContextFrame {
            dim,
            index,
            extent,
            instance,
        });
    }

    /// Pops the innermost frame.
    pub fn pop_dim(&mut self) -> Option<DynamicContextFrame> {
        self.dim_stack.pop()
    }

    /// Returns the innermost frame.
    pub fn current_dim(&self) -> Option<&DynamicContextFrame> {
        self.dim_stack.last()
    }

    /// Returns the nearest frame matching `dim`.
    pub fn find_dim(&self, dim: &DynamicContextDimId) -> Option<&DynamicContextFrame> {
        self.dim_stack.iter().rev().find(|frame| &frame.dim == dim)
    }

    /// Clears all active frames.
    pub fn clear(&mut self) {
        self.dim_stack.clear();
    }
}

/// Computes dense lane index for one dynamic-context state dependency set.
///
/// Returns `None` when dimensions are missing, extents are invalid, or overflow occurs.
pub fn compute_dynamic_context_lane_index(
    state_dims: &[DynamicContextDimId],
    state_extents: &[u32],
    dynamic: &DynamicContext,
) -> Option<usize> {
    if state_dims.len() != state_extents.len() {
        return None;
    }

    let mut flat = 0usize;
    let mut stride = 1usize;

    for (dim, extent) in state_dims.iter().zip(state_extents.iter().copied()) {
        if extent == 0 {
            return None;
        }
        let frame = dynamic.find_dim(dim)?;
        if frame.index >= extent {
            return None;
        }

        let index = usize::try_from(frame.index).ok()?;
        let extent_usize = usize::try_from(extent).ok()?;
        flat = flat.checked_add(index.checked_mul(stride)?)?;
        stride = stride.checked_mul(extent_usize)?;
    }

    Some(flat)
}

#[cfg(test)]
mod tests;
