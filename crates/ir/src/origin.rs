//! Unified semantic node identity (`NodeId`) and provenance (`NodeOrigin`, `SemanticRoleIr`).
//!
//! Separates stable node identity and source/realization provenance from both semantic
//! AST payloads and Typst's `ReferenceLayout` measurements.

use serde::{Deserialize, Serialize};

/// Compact, dense identifier for a semantic node within a [`crate::DocumentIr`].
///
/// `NodeId(0)` ([`NodeId::UNSET`]) represents an unallocated or synthetic node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeId(pub u32);

impl NodeId {
    /// Sentinel value for nodes without a registered `NodeOrigin` / `ReferenceLayout` entry.
    pub const UNSET: Self = Self(0);

    #[inline]
    pub const fn is_unset(&self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub const fn is_set(&self) -> bool {
        self.0 != 0
    }
}

/// Original semantic role of an element before `#show` expansion or target lowering.
///
/// For example, in `#show heading: it => block(fill: gray)[#it.body]`, the resulting node
/// retains `SemanticRoleIr::Heading` alongside its realized container presentation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticRoleIr {
    Title,
    Heading {
        level: usize,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        numbering: Option<String>,
        #[serde(default)]
        outlined: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bookmark: Option<String>,
    },
    Paragraph,
    Quote,
    Figure {
        kind: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bookmark: Option<String>,
    },
    Table,
    TableHeader,
    TableFooter,
    CodeBlock {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        lang: Option<String>,
    },
    Equation {
        is_block: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        numbering: Option<String>,
    },
    List,
    Enum,
    Terms,
    Grid,
    Columns,
    Shape,
    Image,
    CetzCanvas,
    Container,
}

/// Source and realization provenance of a semantic IR node.
///
/// Unlike raw `Span` alone, `typst_location` (`Location::hash()`) uniquely disambiguates
/// multiple instances generated from the same source `Span` (such as `#for` loops or
/// template functions) and enables exact 1-to-N joining of multi-page/multi-column layout fragments.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct NodeOrigin {
    /// Virtual source path relative to workspace/package root (if not detached).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_file: Option<String>,
    /// Byte offset range `(start, end)` within `source_file`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_range: Option<(usize, usize)>,
    /// 1-based `(line, column)` of the start of `source_range` for diagnostics.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_line_col: Option<(usize, usize)>,
    /// Raw Typst `Location::hash()` (`u128` serialized as hex string for JSON/TOML compatibility).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "opt_u128_hex")]
    pub typst_location: Option<u128>,
    /// Enclosing semantic parent `NodeId` at realization time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_parent: Option<NodeId>,
    /// Original Typst semantic role preserved across `#show` rules.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<SemanticRoleIr>,
}

mod opt_u128_hex {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(val: &Option<u128>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match val {
            Some(v) => serializer.serialize_some(&format!("{v:032x}")),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<u128>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt = Option::<String>::deserialize(deserializer)?;
        match opt {
            Some(s) => u128::from_str_radix(&s, 16)
                .map(Some)
                .map_err(serde::de::Error::custom),
            None => Ok(None),
        }
    }
}
