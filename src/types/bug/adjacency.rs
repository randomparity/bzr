use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

/// The closed result emitted by `bzr bug adjacency`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BugAdjacencyResult {
    pub requests: Vec<BugAdjacencyRequest>,
    pub bugs: Vec<BugAdjacencyBug>,
}

/// One positional request outcome in a [`BugAdjacencyResult`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum BugAdjacencyRequest {
    Success {
        requested: String,
        bug_id: u64,
    },
    Failure {
        requested: String,
        error: BugAdjacencyError,
    },
}

/// The only Bugzilla resource failures that can be reported per request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BugAdjacencyError {
    NotFoundAlias,
    NotFoundId,
    Inaccessible,
}

impl BugAdjacencyError {
    #[must_use]
    pub const fn api_code(self) -> u16 {
        match self {
            Self::NotFoundAlias => 100,
            Self::NotFoundId => 101,
            Self::Inaccessible => 102,
        }
    }

    #[must_use]
    pub const fn type_name(self) -> &'static str {
        match self {
            Self::NotFoundAlias | Self::NotFoundId => "not_found",
            Self::Inaccessible => "inaccessible",
        }
    }
}

impl Serialize for BugAdjacencyError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut output = serializer.serialize_struct("BugAdjacencyError", 2)?;
        output.serialize_field("type", self.type_name())?;
        output.serialize_field("api_code", &self.api_code())?;
        output.end()
    }
}

/// A successful canonical bug observation with complete selected adjacency.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BugAdjacencyBug {
    pub id: u64,
    pub summary: Option<String>,
    pub status: Option<String>,
    pub resolution: Option<String>,
    pub product: Option<String>,
    pub version: Option<Vec<String>>,
    pub assigned_to: Option<String>,
    pub last_change_time: Option<String>,
    pub target_milestone: Option<String>,
    pub blocks: Vec<u64>,
    pub depends_on: Vec<u64>,
}

#[cfg(test)]
#[path = "adjacency_tests.rs"]
mod tests;
