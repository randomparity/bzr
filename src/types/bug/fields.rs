use std::collections::HashSet;

/// Which fields the caller asked to include / exclude, as the raw
/// comma-separated `--fields` / `--exclude-fields` values. `Default`
/// (both `None`) means "use the default column set".
#[derive(Debug, Clone, Copy, Default)]
pub struct ColumnSpec<'a> {
    pub include: Option<&'a str>,
    pub exclude: Option<&'a str>,
}

impl<'a> ColumnSpec<'a> {
    /// Build a spec from the `--fields` / `--exclude-fields` values. Single
    /// construction point so the struct can gain fields without updating every
    /// call site.
    pub fn new(include: Option<&'a str>, exclude: Option<&'a str>) -> Self {
        ColumnSpec { include, exclude }
    }
}

/// A built-in Bugzilla bug field selectable by `--fields`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BugField {
    Id,
    Status,
    Priority,
    AssignedTo,
    Summary,
    Severity,
    Product,
    Component,
    Resolution,
    Version,
    Creator,
    CreationTime,
    LastChangeTime,
    Url,
    Whiteboard,
    OpSys,
    RepPlatform,
    Deadline,
    Keywords,
    Blocks,
    DependsOn,
    Cc,
    DupeOf,
    TargetMilestone,
    Flags,
}

impl BugField {
    /// The canonical Bugzilla field name used in REST include/exclude fields.
    pub fn canonical(self) -> &'static str {
        match self {
            BugField::Id => "id",
            BugField::Status => "status",
            BugField::Priority => "priority",
            BugField::AssignedTo => "assigned_to",
            BugField::Summary => "summary",
            BugField::Severity => "severity",
            BugField::Product => "product",
            BugField::Component => "component",
            BugField::Resolution => "resolution",
            BugField::Version => "version",
            BugField::Creator => "creator",
            BugField::CreationTime => "creation_time",
            BugField::LastChangeTime => "last_change_time",
            BugField::Url => "url",
            BugField::Whiteboard => "whiteboard",
            BugField::OpSys => "op_sys",
            BugField::RepPlatform => "rep_platform",
            BugField::Deadline => "deadline",
            BugField::Keywords => "keywords",
            BugField::Blocks => "blocks",
            BugField::DependsOn => "depends_on",
            BugField::Cc => "cc",
            BugField::DupeOf => "dupe_of",
            BugField::TargetMilestone => "target_milestone",
            BugField::Flags => "flags",
        }
    }

    /// Accepted field tokens. The first alias is the canonical Bugzilla field.
    pub fn aliases(self) -> &'static [&'static str] {
        match self {
            BugField::Id => &["id"],
            BugField::Status => &["status"],
            BugField::Priority => &["priority"],
            BugField::AssignedTo => &["assigned_to", "assignee"],
            BugField::Summary => &["summary"],
            BugField::Severity => &["severity"],
            BugField::Product => &["product"],
            BugField::Component => &["component"],
            BugField::Resolution => &["resolution"],
            BugField::Version => &["version"],
            BugField::Creator => &["creator", "reporter"],
            BugField::CreationTime => &["creation_time", "created"],
            BugField::LastChangeTime => &["last_change_time", "updated"],
            BugField::Url => &["url"],
            BugField::Whiteboard => &["whiteboard"],
            BugField::OpSys => &["op_sys"],
            BugField::RepPlatform => &["rep_platform", "platform"],
            BugField::Deadline => &["deadline"],
            BugField::Keywords => &["keywords"],
            BugField::Blocks => &["blocks"],
            BugField::DependsOn => &["depends_on"],
            BugField::Cc => &["cc"],
            BugField::DupeOf => &["dupe_of"],
            BugField::TargetMilestone => &["target_milestone", "milestone"],
            BugField::Flags => &["flags"],
        }
    }

    /// Table/detail label for a selected built-in field.
    pub fn header(self) -> &'static str {
        match self {
            BugField::Id => "ID",
            BugField::Status => "STATUS",
            BugField::Priority => "PRIORITY",
            BugField::AssignedTo => "ASSIGNEE",
            BugField::Summary => "SUMMARY",
            BugField::Severity => "SEVERITY",
            BugField::Product => "PRODUCT",
            BugField::Component => "COMPONENT",
            BugField::Resolution => "RESOLUTION",
            BugField::Version => "VERSION",
            BugField::Creator => "CREATOR",
            BugField::CreationTime => "CREATED",
            BugField::LastChangeTime => "UPDATED",
            BugField::Url => "URL",
            BugField::Whiteboard => "WHITEBOARD",
            BugField::OpSys => "OP_SYS",
            BugField::RepPlatform => "PLATFORM",
            BugField::Deadline => "DEADLINE",
            BugField::Keywords => "KEYWORDS",
            BugField::Blocks => "BLOCKS",
            BugField::DependsOn => "DEPENDS_ON",
            BugField::Cc => "CC",
            BugField::DupeOf => "DUPE_OF",
            BugField::TargetMilestone => "MILESTONE",
            BugField::Flags => "FLAGS",
        }
    }
}

/// Fields shown when `--fields` is not supplied. Order and headers match
/// the historical fixed table.
const DEFAULT_BUG_FIELDS: &[BugField] = &[
    BugField::Id,
    BugField::Status,
    BugField::Priority,
    BugField::AssignedTo,
    BugField::Summary,
];

/// Built-in bug fields recognized by `--fields` / `--exclude-fields`.
pub const BUG_FIELDS: &[BugField] = &[
    BugField::Id,
    BugField::Status,
    BugField::Priority,
    BugField::AssignedTo,
    BugField::Summary,
    BugField::Severity,
    BugField::Product,
    BugField::Component,
    BugField::Resolution,
    BugField::Version,
    BugField::Creator,
    BugField::CreationTime,
    BugField::LastChangeTime,
    BugField::Url,
    BugField::Whiteboard,
    BugField::OpSys,
    BugField::RepPlatform,
    BugField::Deadline,
    BugField::Keywords,
    BugField::Blocks,
    BugField::DependsOn,
    BugField::Cc,
    BugField::DupeOf,
    BugField::TargetMilestone,
    BugField::Flags,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectedBugField<'a> {
    BuiltIn(BugField),
    Custom(&'a str),
}

impl<'a> SelectedBugField<'a> {
    pub fn key(self) -> &'a str {
        match self {
            SelectedBugField::BuiltIn(field) => field.canonical(),
            SelectedBugField::Custom(name) => name,
        }
    }

    pub fn header(self) -> String {
        match self {
            SelectedBugField::BuiltIn(field) => field.header().to_string(),
            SelectedBugField::Custom(name) => name.to_ascii_uppercase(),
        }
    }
}

pub struct FieldPartition<'a> {
    pub ordered: Vec<SelectedBugField<'a>>,
    pub custom: Vec<&'a str>,
    pub unknown: Vec<&'a str>,
}

/// Resolve a single field token to its built-in field, case-insensitively.
pub fn resolve_bug_field(token: &str) -> Option<BugField> {
    let token = token.trim().to_ascii_lowercase();
    BUG_FIELDS
        .iter()
        .copied()
        .find(|field| field.aliases().contains(&token.as_str()))
}

/// Translate a comma-separated field list (which may use aliases such as
/// `assignee` or `updated`) into canonical Bugzilla field names for the
/// server's `include_fields` / `exclude_fields` parameters. Unknown tokens
/// (e.g. custom `cf_*` fields) pass through unchanged. Empty input or an
/// all-empty list yields `None`.
pub fn canonical_field_list(fields: Option<&str>) -> Option<String> {
    let fields = fields?;
    let mut out: Vec<&str> = Vec::new();
    for token in fields.split(',') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        match resolve_bug_field(token) {
            Some(field) => out.push(field.canonical()),
            None => out.push(token),
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out.join(","))
    }
}

pub fn default_selected_fields() -> Vec<SelectedBugField<'static>> {
    DEFAULT_BUG_FIELDS
        .iter()
        .copied()
        .map(SelectedBugField::BuiltIn)
        .collect()
}

pub fn is_custom_bug_field(token: &str) -> bool {
    token.starts_with("cf_")
}

/// Split a comma list into built-in fields, custom fields, and unknown tokens,
/// trimming and skipping blanks. Shared by renderers and pre-flight validators
/// so output and validation use the same field taxonomy.
pub fn partition_include(list: &str) -> FieldPartition<'_> {
    let mut partition = FieldPartition {
        ordered: Vec::new(),
        custom: Vec::new(),
        unknown: Vec::new(),
    };
    let mut seen = HashSet::new();
    for token in list.split(',') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        match resolve_bug_field(token) {
            Some(field) => {
                if seen.insert(field.canonical()) {
                    partition.ordered.push(SelectedBugField::BuiltIn(field));
                }
            }
            None if is_custom_bug_field(token) => {
                if seen.insert(token) {
                    partition.ordered.push(SelectedBugField::Custom(token));
                    partition.custom.push(token);
                }
            }
            None => partition.unknown.push(token),
        }
    }
    partition
}

pub fn selected_keys<'a>(fields: &[SelectedBugField<'a>]) -> HashSet<&'a str> {
    fields.iter().map(|field| (*field).key()).collect()
}

fn excluded_keys(exclude: Option<&str>) -> HashSet<&str> {
    let Some(list) = exclude else {
        return HashSet::new();
    };
    partition_include(list)
        .ordered
        .iter()
        .map(|field| (*field).key())
        .collect()
}

/// Apply `exclude` to `fields` in place, dropping matching built-in or custom
/// selections.
pub fn apply_exclude(fields: &mut Vec<SelectedBugField<'_>>, exclude: Option<&str>) {
    let excluded = excluded_keys(exclude);
    fields.retain(|field| !excluded.contains((*field).key()));
}

/// The canonical exclude key set: each `--exclude-fields` token resolved to its
/// serialized key. Unknown non-custom tokens name no key and are dropped here,
/// matching table mode's exclude behavior.
pub fn canonical_excludes(exclude: Option<&str>) -> Vec<&str> {
    match exclude {
        None => Vec::new(),
        Some(list) => partition_include(list)
            .ordered
            .iter()
            .map(|field| (*field).key())
            .collect(),
    }
}

/// Whether a detail-view field should render given `spec`. With no include
/// list, every field shows (minus excludes). Tokens are matched against the
/// field registry so `assignee`/`assigned_to` etc. are equivalent. Fields
/// with no registry entry always show by default.
pub fn field_selected(spec: ColumnSpec<'_>, field: &str) -> bool {
    let Some(target) = resolve_bug_field(field) else {
        return true;
    };
    let matches = |list: &str| {
        list.split(',')
            .filter_map(resolve_bug_field)
            .any(|field| field == target)
    };
    let included = spec.include.is_none_or(matches);
    let excluded = spec.exclude.is_some_and(matches);
    included && !excluded
}

pub fn selected_custom_detail_fields(spec: ColumnSpec<'_>) -> Vec<&str> {
    let Some(include) = spec.include else {
        return Vec::new();
    };
    let excluded = excluded_keys(spec.exclude);
    partition_include(include)
        .custom
        .into_iter()
        .filter(|name| !excluded.contains(name))
        .collect()
}

#[cfg(test)]
#[path = "fields_tests.rs"]
mod tests;
