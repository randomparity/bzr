#![expect(clippy::unwrap_used)]

use super::*;

#[test]
fn output_format_from_str_valid() {
    assert_eq!(
        "table".parse::<OutputFormat>().unwrap(),
        OutputFormat::Table
    );
    assert_eq!("json".parse::<OutputFormat>().unwrap(), OutputFormat::Json);
    assert_eq!(
        "ndjson".parse::<OutputFormat>().unwrap(),
        OutputFormat::Ndjson
    );
}

#[test]
fn output_format_is_json_family() {
    assert!(OutputFormat::Json.is_json_family());
    assert!(OutputFormat::Ndjson.is_json_family());
    assert!(!OutputFormat::Table.is_json_family());
}

#[test]
fn output_format_from_str_invalid() {
    let err = "xml".parse::<OutputFormat>().unwrap_err();
    assert!(err.contains("invalid output format"));
    assert!(err.contains("ndjson"));
}

#[test]
fn output_format_default_is_table() {
    assert_eq!(OutputFormat::default(), OutputFormat::Table);
}

#[test]
fn sort_direction_from_str_and_keyword() {
    assert_eq!("asc".parse::<SortDirection>().unwrap(), SortDirection::Asc);
    assert_eq!("ASC".parse::<SortDirection>().unwrap(), SortDirection::Asc);
    assert_eq!(
        "ascending".parse::<SortDirection>().unwrap(),
        SortDirection::Asc
    );
    assert_eq!(
        "desc".parse::<SortDirection>().unwrap(),
        SortDirection::Desc
    );
    assert_eq!(
        "DESCENDING".parse::<SortDirection>().unwrap(),
        SortDirection::Desc
    );
    assert!("sideways".parse::<SortDirection>().is_err());
    assert_eq!(SortDirection::Asc.keyword(), "ASC");
    assert_eq!(SortDirection::Desc.keyword(), "DESC");
    assert_eq!(SortDirection::default(), SortDirection::Asc);
}
