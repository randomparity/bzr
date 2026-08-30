use std::fmt;
use std::str::FromStr;

use serde::de::{self, Deserializer, IgnoredAny, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize, Serializer};

use super::Bug;

/// Max bug ids packed into one `?id=` REST request during link traversal.
pub const LINKS_ID_CHUNK: usize = 100;
/// Max distinct related bugs a single `bug links` walk will visit.
pub const LINKS_MAX_NODES: usize = 1000;
/// The exact field set `bug links` requests, isolated from `BUG_DEFAULT_FIELDS`.
pub const LINKS_INCLUDE_FIELDS: &str =
    "id,summary,status,depends_on,blocks,dupe_of,duplicates,regressed_by,regressions";

/// One of the six Bugzilla bug relationship types, in three inverse pairs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkRelation {
    DependsOn,
    Blocks,
    DupeOf,
    Duplicates,
    RegressedBy,
    Regressions,
}

impl LinkRelation {
    /// All relations in the fixed traversal/emit order.
    pub const ALL: [LinkRelation; 6] = [
        LinkRelation::DependsOn,
        LinkRelation::Blocks,
        LinkRelation::DupeOf,
        LinkRelation::Duplicates,
        LinkRelation::RegressedBy,
        LinkRelation::Regressions,
    ];

    /// The Bugzilla wire field name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            LinkRelation::DependsOn => "depends_on",
            LinkRelation::Blocks => "blocks",
            LinkRelation::DupeOf => "dupe_of",
            LinkRelation::Duplicates => "duplicates",
            LinkRelation::RegressedBy => "regressed_by",
            LinkRelation::Regressions => "regressions",
        }
    }

    /// Fixed in/out direction for graph orientation (see ADR-0006).
    #[must_use]
    pub fn direction(self) -> LinkDirection {
        match self {
            LinkRelation::DependsOn | LinkRelation::DupeOf | LinkRelation::RegressedBy => {
                LinkDirection::Out
            }
            LinkRelation::Blocks | LinkRelation::Duplicates | LinkRelation::Regressions => {
                LinkDirection::In
            }
        }
    }
}

impl fmt::Display for LinkRelation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for LinkRelation {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        LinkRelation::ALL
            .into_iter()
            .find(|r| r.as_str() == s)
            .ok_or_else(|| {
                format!(
                    "invalid relation '{s}': expected one of \
                     depends_on, blocks, dupe_of, duplicates, regressed_by, regressions"
                )
            })
    }
}

/// Edge orientation relative to the root bug.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkDirection {
    In,
    Out,
}

impl LinkDirection {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            LinkDirection::In => "in",
            LinkDirection::Out => "out",
        }
    }
}

impl Serialize for LinkDirection {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

/// One emitted relationship record.
#[derive(Debug, Clone)]
pub struct BugLink {
    pub id: u64,
    pub relation: LinkRelation,
    pub direction: LinkDirection,
    pub depth: u32,
    pub summary: Option<String>,
    pub status: Option<String>,
}

impl Serialize for BugLink {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(6))?;
        map.serialize_entry("id", &self.id)?;
        map.serialize_entry("relation", self.relation.as_str())?;
        map.serialize_entry("direction", &self.direction)?;
        map.serialize_entry("depth", &self.depth)?;
        map.serialize_entry("summary", &self.summary)?;
        map.serialize_entry("status", &self.status)?;
        map.end()
    }
}

/// Isolated relationship-fetch shape — never the global [`Bug`] type. Requesting
/// exactly these fields keeps the global default-field list and other commands
/// untouched (see ADR-0006).
#[derive(Debug, Clone, Deserialize)]
pub struct BugLinksNode {
    pub id: u64,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub depends_on: Vec<u64>,
    #[serde(default)]
    pub blocks: Vec<u64>,
    #[serde(default)]
    pub dupe_of: Option<u64>,
    #[serde(default, deserialize_with = "deserialize_relationship_ids")]
    pub duplicates: Vec<u64>,
    #[serde(default)]
    pub regressed_by: Vec<u64>,
    #[serde(default)]
    pub regressions: Vec<u64>,
}

#[derive(Clone, Copy)]
struct PositiveRelationshipId(u64);

impl<'de> Deserialize<'de> for PositiveRelationshipId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_u64(PositiveRelationshipIdVisitor)
    }
}

struct PositiveRelationshipIdVisitor;

impl Visitor<'_> for PositiveRelationshipIdVisitor {
    type Value = PositiveRelationshipId;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a positive integer relationship ID")
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
        if value == 0 {
            return Err(E::custom("expected a positive integer relationship ID"));
        }
        Ok(PositiveRelationshipId(value))
    }
}

struct RelationshipIdVisitor;

impl<'de> Visitor<'de> for RelationshipIdVisitor {
    type Value = PositiveRelationshipId;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            "a positive integer relationship ID or an object containing a positive integer bug_id",
        )
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
        PositiveRelationshipIdVisitor.visit_u64(value)
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut bug_id = None;
        while let Some(key) = map.next_key::<String>()? {
            if key == "bug_id" {
                if bug_id.is_some() {
                    return Err(de::Error::duplicate_field("bug_id"));
                }
                bug_id = Some(map.next_value::<PositiveRelationshipId>()?);
            } else {
                map.next_value::<IgnoredAny>()?;
            }
        }
        bug_id.ok_or_else(|| {
            de::Error::custom(
                "expected an object containing a positive integer relationship ID in bug_id",
            )
        })
    }
}

impl<'de> Deserialize<'de> for RelationshipIdVisitorValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer
            .deserialize_any(RelationshipIdVisitor)
            .map(|id| RelationshipIdVisitorValue(id.0))
    }
}

struct RelationshipIdVisitorValue(u64);

fn deserialize_relationship_ids<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<u64>, D::Error> {
    Vec::<RelationshipIdVisitorValue>::deserialize(deserializer)
        .map(|ids| ids.into_iter().map(|id| id.0).collect())
}

impl BugLinksNode {
    /// Build a link node from a full [`Bug`] (the XML-RPC fallback path). Only
    /// the three core relations exist on `Bug`; BMO fields stay empty.
    #[must_use]
    pub fn from_bug(bug: &Bug) -> Self {
        BugLinksNode {
            id: bug.id,
            summary: bug.summary.clone(),
            status: bug.status.clone(),
            depends_on: bug.depends_on.clone(),
            blocks: bug.blocks.clone(),
            dupe_of: bug.dupe_of,
            duplicates: Vec::new(),
            regressed_by: Vec::new(),
            regressions: Vec::new(),
        }
    }

    fn ids_for(&self, relation: LinkRelation) -> Vec<u64> {
        match relation {
            LinkRelation::DependsOn => self.depends_on.clone(),
            LinkRelation::Blocks => self.blocks.clone(),
            LinkRelation::DupeOf => self.dupe_of.into_iter().collect(),
            LinkRelation::Duplicates => self.duplicates.clone(),
            LinkRelation::RegressedBy => self.regressed_by.clone(),
            LinkRelation::Regressions => self.regressions.clone(),
        }
    }

    /// Adjacency in fixed relation order, each relation's ids ascending. With a
    /// `filter`, only that relation's edges are produced.
    #[must_use]
    pub fn edges(&self, filter: Option<LinkRelation>) -> Vec<(LinkRelation, u64)> {
        let mut edges = Vec::new();
        for relation in LinkRelation::ALL {
            if filter.is_some_and(|f| f != relation) {
                continue;
            }
            let mut ids = self.ids_for(relation);
            ids.sort_unstable();
            for id in ids {
                edges.push((relation, id));
            }
        }
        edges
    }
}

#[cfg(test)]
#[path = "links_tests.rs"]
mod tests;
