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
pub(crate) use fields::BUG_SEARCH_DEFAULT_FIELDS;
pub use fields::{
    apply_exclude, canonical_excludes, canonical_field_list, default_selected_fields,
    field_selected, partition_include, selected_custom_detail_fields, selected_keys, BugField,
    ColumnSpec, SelectedBugField, BUG_FIELDS,
};

pub use links::{
    BugLink, BugLinksNode, LinkRelation, LINKS_ID_CHUNK, LINKS_INCLUDE_FIELDS, LINKS_MAX_NODES,
};
pub use payload::{
    CommentUpdate, CreateBugParams, ExtraBugFields, IdListUpdate, StringListUpdate, UpdateBugParams,
};
pub use search::{
    partition_filters, FieldMapping, FilterField, NegationOp, Overrides, SearchParams,
    FIELD_MAPPINGS,
};

const BUG_BUILT_IN_FIELD_COUNT: usize = 29;

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
    pub component: Option<Vec<String>>,
    pub version: Option<Vec<String>>,
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
    pub platform: Option<String>,
    pub target_milestone: Option<String>,
    pub groups: Vec<String>,
    pub estimated_time: Option<f64>,
    pub remaining_time: Option<f64>,
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
    #[serde(default, deserialize_with = "deserialize_optional_string_list")]
    component: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_optional_string_list")]
    version: Option<Vec<String>>,
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
    #[serde(default, deserialize_with = "deserialize_cc_string_list")]
    cc: Vec<String>,
    #[serde(default)]
    op_sys: Option<String>,
    #[serde(default)]
    platform: Option<String>,
    #[serde(default)]
    target_milestone: Option<String>,
    #[serde(default)]
    groups: Vec<String>,
    #[serde(default)]
    estimated_time: Option<f64>,
    #[serde(default)]
    remaining_time: Option<f64>,
    #[serde(default)]
    flags: Vec<Flag>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum StringOrList {
    String(String),
    List(Vec<String>),
}

pub(crate) fn deserialize_optional_string_list<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    StringOrList::deserialize(deserializer).map(|value| {
        Some(match value {
            StringOrList::String(value) if value.is_empty() => return None,
            StringOrList::String(value) => vec![value],
            StringOrList::List(values) => values,
        })
    })
}

/// One `cc` list member as served by Bugzilla REST. Upstream servers send
/// login-name strings; bugzilla.redhat.com sends the `cc_detail` user objects
/// (`{name, email, real_name, id, ...}`) to authenticated clients. Accept both
/// wire shapes so `bug view` deserializes against either server.
#[derive(Deserialize)]
#[serde(untagged)]
enum CcEntry {
    String(String),
    Object {
        name: Option<String>,
        email: Option<String>,
    },
}

impl CcEntry {
    fn into_string(self) -> Option<String> {
        match self {
            Self::String(value) => Some(value),
            // `name` is the Bugzilla login; `email` is Red Hat's documented
            // cc_detail equivalent ("currently the same as the login name").
            Self::Object { name, email } => name.or(email),
        }
    }
}

fn deserialize_cc_string_list<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let entries = Vec::<CcEntry>::deserialize(deserializer)?;
    entries
        .into_iter()
        .map(|entry| {
            entry.into_string().ok_or_else(|| {
                <D::Error as serde::de::Error>::custom(
                    "cc member object has neither `name` nor `email`",
                )
            })
        })
        .collect()
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
            platform: wire.platform,
            target_milestone: wire.target_milestone,
            groups: wire.groups,
            estimated_time: wire.estimated_time,
            remaining_time: wire.remaining_time,
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
        map.serialize_entry("platform", &self.platform)?;
        map.serialize_entry("target_milestone", &self.target_milestone)?;
        map.serialize_entry("groups", &self.groups)?;
        if let Some(estimated_time) = self.estimated_time {
            map.serialize_entry("estimated_time", &estimated_time)?;
        }
        if let Some(remaining_time) = self.remaining_time {
            map.serialize_entry("remaining_time", &remaining_time)?;
        }
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
