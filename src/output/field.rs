use tabled::{Table, Tabled};

use super::common::print_formatted;
use crate::types::{FieldValue, OutputFormat};

#[derive(Tabled)]
struct FieldValueRow {
    #[tabled(rename = "NAME")]
    name: String,
    #[tabled(rename = "ACTIVE")]
    active: String,
    #[tabled(rename = "CAN CHANGE TO")]
    can_change_to: String,
}

#[expect(clippy::print_stdout)]
pub fn print_field_values(values: &[FieldValue], field_name: &str, format: OutputFormat) {
    print_formatted(values, format, |values| {
        if values.is_empty() {
            println!("No values for field '{field_name}'.");
            return;
        }
        let rows: Vec<FieldValueRow> = values
            .iter()
            .map(|v| {
                let transitions = v
                    .can_change_to
                    .as_ref()
                    .map(|t| {
                        t.iter()
                            .map(|s| s.name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                FieldValueRow {
                    name: v.name.clone(),
                    active: if v.is_active {
                        "yes".into()
                    } else {
                        "no".into()
                    },
                    can_change_to: transitions,
                }
            })
            .collect();
        println!("{}", Table::new(rows));
    });
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use crate::types::{FieldValue, StatusTransition};

    #[test]
    fn print_field_values_json_empty() {
        let values: Vec<FieldValue> = vec![];
        let json = serde_json::to_string_pretty(&values).unwrap();
        assert_eq!(json, "[]");
    }

    #[test]
    fn print_field_values_json_with_transitions() {
        let values = vec![FieldValue {
            name: "NEW".into(),
            sort_key: 0,
            is_active: true,
            can_change_to: Some(vec![
                StatusTransition {
                    name: "ASSIGNED".into(),
                },
                StatusTransition {
                    name: "RESOLVED".into(),
                },
            ]),
        }];
        let json = serde_json::to_string_pretty(&values).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed[0]["name"], "NEW");
        assert_eq!(parsed[0]["is_active"], true);
        let transitions = parsed[0]["can_change_to"].as_array().unwrap();
        assert_eq!(transitions.len(), 2);
        assert_eq!(transitions[0]["name"], "ASSIGNED");
    }

    #[test]
    fn print_field_values_json_active_and_inactive() {
        let values = vec![
            FieldValue {
                name: "NEW".into(),
                sort_key: 0,
                is_active: true,
                can_change_to: Some(vec![StatusTransition {
                    name: "ASSIGNED".into(),
                }]),
            },
            FieldValue {
                name: "CLOSED".into(),
                sort_key: 1,
                is_active: false,
                can_change_to: None,
            },
        ];
        let json = serde_json::to_string_pretty(&values).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed[0]["name"], "NEW");
        assert_eq!(parsed[0]["is_active"], true);
        assert_eq!(parsed[1]["name"], "CLOSED");
        assert_eq!(parsed[1]["is_active"], false);
        assert!(parsed[1]["can_change_to"].is_null());
    }
}
