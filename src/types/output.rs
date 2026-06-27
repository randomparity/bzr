use std::str::FromStr;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OutputFormat {
    #[default]
    Table,
    Json,
    /// Newline-delimited JSON: array outputs emit one compact value per line;
    /// single objects emit one compact line. Streaming-friendly for agents.
    Ndjson,
}

impl OutputFormat {
    /// Whether this format is a machine-readable JSON family (`json` or
    /// `ndjson`) as opposed to the human `table`. The two JSON families share
    /// every routing decision — serialization, error objects, stderr-only
    /// notes — and differ only in how the JSON is laid out, so call sites that
    /// branch "machine vs human" test this rather than spelling out both arms.
    #[must_use]
    pub fn is_json_family(self) -> bool {
        match self {
            OutputFormat::Json | OutputFormat::Ndjson => true,
            OutputFormat::Table => false,
        }
    }
}

impl FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "table" => Ok(OutputFormat::Table),
            "json" => Ok(OutputFormat::Json),
            "ndjson" => Ok(OutputFormat::Ndjson),
            _ => Err(format!(
                "invalid output format '{s}': expected 'table', 'json', or 'ndjson'"
            )),
        }
    }
}

/// Progress-stream format for the `--progress` flag. Only `ndjson` exists; the
/// flag takes a value so a future format is a non-breaking addition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressFormat {
    Ndjson,
}

impl std::fmt::Display for ProgressFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProgressFormat::Ndjson => write!(f, "ndjson"),
        }
    }
}

impl FromStr for ProgressFormat {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "ndjson" => Ok(ProgressFormat::Ndjson),
            _ => Err(format!("invalid progress format '{s}': expected 'ndjson'")),
        }
    }
}

/// Sort direction for the `--order` flag on listing commands.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SortDirection {
    #[default]
    Asc,
    Desc,
}

impl SortDirection {
    /// The Bugzilla `order` keyword for this direction.
    #[must_use]
    pub fn keyword(self) -> &'static str {
        match self {
            SortDirection::Asc => "ASC",
            SortDirection::Desc => "DESC",
        }
    }
}

impl FromStr for SortDirection {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "asc" | "ascending" => Ok(SortDirection::Asc),
            "desc" | "descending" => Ok(SortDirection::Desc),
            _ => Err(format!("invalid order '{s}': expected 'asc' or 'desc'")),
        }
    }
}

#[cfg(test)]
#[path = "output_tests.rs"]
mod tests;
