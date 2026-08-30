use std::collections::BTreeMap;

use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use super::flag::Flag;

mod adjacency;
mod fields;
mod links;
mod payload;
mod search;

pub use adjacency::{BugAdjacencyBug, BugAdjacencyError, BugAdjacencyRequest, BugAdjacencyResult};
pub use fields::{
    apply_exclude, canonical_excludes, canonical_field_list, default_selected_fields,
    field_selected, partition_include, selected_custom_detail_fields, selected_keys, BugField,
    ColumnSpec, SelectedBugField, BUG_FIELDS,
};

pub use links::{
    BugLink, BugLinksNode, LinkRelation, LINKS_ID_CHUNK, LINKS_INCLUDE_FIELDS, LINKS_MAX_NODES,
};
pub use payload::{
    CommentUpdate, CreateBugParams, IdListUpdate, StringListUpdate, UpdateBugParams,
};
pub use search::{
    partition_filters, FieldMapping, FilterField, NegationOp, Overrides, SearchParams,
    FIELD_MAPPINGS,
};

const BUG_BUILT_IN_FIELD_COUNT: usize = 25;

fn is_custom_field_name(name: &str) -> bool {
    name.starts_with("cf_")
}

#[derive(Debug)]
#[non_exhaustive]
pub struct Bug {
    pub id: u64,
    pub summary: Option<String>,
    pub status: Option<String>,
    pub resolution: Option<String>,
    pub dupe_of: Option<u64>,
    pub deadline: Option<String>,
    pub product: Option<String>,
    pub component: Option<String>,
    pub version: Option<String>,
    pub assigned_to: Option<String>,
    pub priority: Option<String>,
    pub severity: Option<String>,
    pub creation_time: Option<String>,
    pub last_change_time: Option<String>,
    pub creator: Option<String>,
    pub url: Option<String>,
    pub whiteboard: Option<String>,
    pub keywords: Vec<String>,
    pub blocks: Vec<u64>,
    pub depends_on: Vec<u64>,
    pub cc: Vec<String>,
    pub op_sys: Option<String>,
    pub rep_platform: Option<String>,
    pub target_milestone: Option<String>,
    pub flags: Vec<Flag>,
    pub custom_fields: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct BugWire {
    id: u64,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    resolution: Option<String>,
    #[serde(default)]
    dupe_of: Option<u64>,
    #[serde(default)]
    deadline: Option<String>,
    #[serde(default)]
    product: Option<String>,
    #[serde(default)]
    component: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    assigned_to: Option<String>,
    #[serde(default)]
    priority: Option<String>,
    #[serde(default)]
    severity: Option<String>,
    #[serde(default)]
    creation_time: Option<String>,
    #[serde(default)]
    last_change_time: Option<String>,
    #[serde(default)]
    creator: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    whiteboard: Option<String>,
    #[serde(default)]
    keywords: Vec<String>,
    #[serde(default)]
    blocks: Vec<u64>,
    #[serde(default)]
    depends_on: Vec<u64>,
    #[serde(default)]
    cc: Vec<String>,
    #[serde(default)]
    op_sys: Option<String>,
    #[serde(default)]
    rep_platform: Option<String>,
    #[serde(default)]
    target_milestone: Option<String>,
    #[serde(default)]
    flags: Vec<Flag>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

impl From<BugWire> for Bug {
    fn from(wire: BugWire) -> Self {
        Bug {
            id: wire.id,
            summary: wire.summary,
            status: wire.status,
            resolution: wire.resolution,
            dupe_of: wire.dupe_of,
            deadline: wire.deadline,
            product: wire.product,
            component: wire.component,
            version: wire.version,
            assigned_to: wire.assigned_to,
            priority: wire.priority,
            severity: wire.severity,
            creation_time: wire.creation_time,
            last_change_time: wire.last_change_time,
            creator: wire.creator,
            url: wire.url,
            whiteboard: wire.whiteboard,
            keywords: wire.keywords,
            blocks: wire.blocks,
            depends_on: wire.depends_on,
            cc: wire.cc,
            op_sys: wire.op_sys,
            rep_platform: wire.rep_platform,
            target_milestone: wire.target_milestone,
            flags: wire.flags,
            custom_fields: wire
                .extra
                .into_iter()
                .filter(|(name, _)| is_custom_field_name(name))
                .collect(),
        }
    }
}

impl<'de> Deserialize<'de> for Bug {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        BugWire::deserialize(deserializer).map(Self::from)
    }
}

impl Serialize for Bug {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let custom_field_count = self
            .custom_fields
            .keys()
            .filter(|name| is_custom_field_name(name))
            .count();
        let mut map =
            serializer.serialize_map(Some(BUG_BUILT_IN_FIELD_COUNT + custom_field_count))?;
        map.serialize_entry("id", &self.id)?;
        map.serialize_entry("summary", &self.summary)?;
        map.serialize_entry("status", &self.status)?;
        map.serialize_entry("resolution", &self.resolution)?;
        map.serialize_entry("dupe_of", &self.dupe_of)?;
        map.serialize_entry("deadline", &self.deadline)?;
        map.serialize_entry("product", &self.product)?;
        map.serialize_entry("component", &self.component)?;
        map.serialize_entry("version", &self.version)?;
        map.serialize_entry("assigned_to", &self.assigned_to)?;
        map.serialize_entry("priority", &self.priority)?;
        map.serialize_entry("severity", &self.severity)?;
        map.serialize_entry("creation_time", &self.creation_time)?;
        map.serialize_entry("last_change_time", &self.last_change_time)?;
        map.serialize_entry("creator", &self.creator)?;
        map.serialize_entry("url", &self.url)?;
        map.serialize_entry("whiteboard", &self.whiteboard)?;
        map.serialize_entry("keywords", &self.keywords)?;
        map.serialize_entry("blocks", &self.blocks)?;
        map.serialize_entry("depends_on", &self.depends_on)?;
        map.serialize_entry("cc", &self.cc)?;
        map.serialize_entry("op_sys", &self.op_sys)?;
        map.serialize_entry("rep_platform", &self.rep_platform)?;
        map.serialize_entry("target_milestone", &self.target_milestone)?;
        map.serialize_entry("flags", &self.flags)?;
        for (name, value) in self
            .custom_fields
            .iter()
            .filter(|(name, _)| is_custom_field_name(name))
        {
            map.serialize_entry(name, value)?;
        }
        map.end()
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct HistoryEntry {
    pub who: String,
    pub when: String,
    pub changes: Vec<FieldChange>,
}

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct FieldChange {
    pub field_name: String,
    #[serde(default)]
    pub removed: Option<String>,
    #[serde(default)]
    pub added: Option<String>,
    #[serde(default)]
    pub attachment_id: Option<u64>,
}

/// One flattened `bug history` change record: a single field mutation, as
/// emitted in `--json` / `--output ndjson` output. A history entry with N
/// changed fields expands to N records sharing `when`/`who`/`comment_id`. See
/// ADR 0008 and `schemas/history.json` for the published contract.
#[derive(Debug, Serialize)]
pub struct HistoryRecord {
    pub when: String,
    pub who: String,
    pub field: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub comment_id: Option<u64>,
}

#[cfg(test)]
#[path = "bug_tests.rs"]
mod tests;
