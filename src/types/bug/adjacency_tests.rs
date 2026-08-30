#![expect(clippy::unwrap_used)]

use super::*;

#[test]
fn adjacency_error_serialization_is_limited_to_the_three_supported_codes() {
    let values = [
        (BugAdjacencyError::NotFoundAlias, "not_found", 100),
        (BugAdjacencyError::NotFoundId, "not_found", 101),
        (BugAdjacencyError::Inaccessible, "inaccessible", 102),
    ];

    for (error, type_name, api_code) in values {
        assert_eq!(
            serde_json::to_value(error).unwrap(),
            serde_json::json!({"type": type_name, "api_code": api_code})
        );
    }
}

#[test]
fn adjacency_request_outcomes_are_exclusive_and_preserve_request_text() {
    let result = BugAdjacencyResult {
        requests: vec![
            BugAdjacencyRequest::Success {
                requested: "00123".into(),
                bug_id: 123,
            },
            BugAdjacencyRequest::Failure {
                requested: "missing-alias".into(),
                error: BugAdjacencyError::NotFoundAlias,
            },
        ],
        bugs: vec![],
    };

    assert_eq!(
        serde_json::to_value(result).unwrap(),
        serde_json::json!({
            "requests": [
                {"requested": "00123", "bug_id": 123},
                {
                    "requested": "missing-alias",
                    "error": {"type": "not_found", "api_code": 100}
                }
            ],
            "bugs": []
        })
    );
}
