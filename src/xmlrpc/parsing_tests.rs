#![expect(clippy::unwrap_used)]

use crate::xmlrpc::value::Value;

use super::parse_response;

#[test]
fn parse_success_response_with_struct() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
    <methodResponse>
      <params>
        <param>
          <value>
            <struct>
              <member>
                <name>bugs</name>
                <value>
                  <array>
                    <data>
                      <value>
                        <struct>
                          <member>
                            <name>id</name>
                            <value><int>12345</int></value>
                          </member>
                          <member>
                            <name>summary</name>
                            <value><string>Test bug</string></value>
                          </member>
                        </struct>
                      </value>
                    </data>
                  </array>
                </value>
              </member>
            </struct>
          </value>
        </param>
      </params>
    </methodResponse>"#;

    let result = parse_response(xml).unwrap();
    let top = result.as_struct().unwrap();
    let bugs = top.get("bugs").unwrap().as_array().unwrap();
    assert_eq!(bugs.len(), 1);
    let bug = bugs[0].as_struct().unwrap();
    assert_eq!(bug.get("id").unwrap().as_i64().unwrap(), 12345);
    assert_eq!(bug.get("summary").unwrap().as_str().unwrap(), "Test bug");
}

#[test]
fn parse_fault_response() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
    <methodResponse>
      <fault>
        <value>
          <struct>
            <member>
              <name>faultCode</name>
              <value><int>102</int></value>
            </member>
            <member>
              <name>faultString</name>
              <value><string>Access denied</string></value>
            </member>
          </struct>
        </value>
      </fault>
    </methodResponse>"#;

    let err = parse_response(xml).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("102"), "should contain fault code: {msg}");
    assert!(
        msg.contains("Access denied"),
        "should contain fault message: {msg}"
    );
}

#[test]
fn parse_response_with_double_and_datetime() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
    <methodResponse>
      <params>
        <param>
          <value>
            <struct>
              <member>
                <name>score</name>
                <value><double>42.5</double></value>
              </member>
              <member>
                <name>when</name>
                <value><dateTime.iso8601>20250101T12:00:00</dateTime.iso8601></value>
              </member>
            </struct>
          </value>
        </param>
      </params>
    </methodResponse>"#;

    let result = parse_response(xml).unwrap();
    let s = result.as_struct().unwrap();
    let score = s.get("score").unwrap().as_f64().unwrap();
    assert!((score - 42.5).abs() < f64::EPSILON);
    assert_eq!(
        s.get("when").unwrap(),
        &Value::DateTime("20250101T12:00:00".into())
    );
}

#[test]
fn parse_bare_text_as_string() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
    <methodResponse>
      <params>
        <param>
          <value>hello world</value>
        </param>
      </params>
    </methodResponse>"#;

    let result = parse_response(xml).unwrap();
    assert_eq!(result.as_str().unwrap(), "hello world");
}

#[test]
fn parse_empty_struct() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
    <methodResponse>
      <params>
        <param>
          <value><struct></struct></value>
        </param>
      </params>
    </methodResponse>"#;

    let result = parse_response(xml).unwrap();
    let s = result.as_struct().unwrap();
    assert!(s.is_empty());
}

#[test]
fn parse_empty_array() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
    <methodResponse>
      <params>
        <param>
          <value><array><data></data></array></value>
        </param>
      </params>
    </methodResponse>"#;

    let result = parse_response(xml).unwrap();
    let a = result.as_array().unwrap();
    assert!(a.is_empty());
}

#[test]
fn parse_i4_type() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
    <methodResponse>
      <params>
        <param>
          <value><i4>42</i4></value>
        </param>
      </params>
    </methodResponse>"#;

    let result = parse_response(xml).unwrap();
    assert_eq!(result.as_i64().unwrap(), 42);
}

#[test]
fn parse_boolean_values() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
    <methodResponse>
      <params>
        <param>
          <value>
            <struct>
              <member>
                <name>yes</name>
                <value><boolean>1</boolean></value>
              </member>
              <member>
                <name>no</name>
                <value><boolean>0</boolean></value>
              </member>
            </struct>
          </value>
        </param>
      </params>
    </methodResponse>"#;

    let result = parse_response(xml).unwrap();
    let s = result.as_struct().unwrap();
    assert!(s.get("yes").unwrap().as_bool().unwrap());
    assert!(!s.get("no").unwrap().as_bool().unwrap());
}

#[test]
fn parse_base64_value() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
    <methodResponse>
      <params>
        <param>
          <value><base64>SGVsbG8=</base64></value>
        </param>
      </params>
    </methodResponse>"#;

    let result = parse_response(xml).unwrap();
    assert!(
        matches!(&result, Value::Base64(bytes) if bytes == b"Hello"),
        "expected Base64(Hello), got {result:?}"
    );
}

// CDATA sections inside text content must be appended to the accumulated
// string, not silently dropped.
#[test]
fn parse_string_with_cdata_section() {
    let xml = r#"<methodResponse><params><param><value>
        <string><![CDATA[contains <special> & "characters"]]></string>
    </value></param></params></methodResponse>"#;
    let result = parse_response(xml).unwrap();
    assert_eq!(
        result.as_str().unwrap(),
        r#"contains <special> & "characters""#
    );
}

// Empty `<value></value>` is treated as an empty string.
#[test]
fn parse_empty_value_returns_empty_string() {
    let xml = r"<methodResponse><params><param><value></value></param></params></methodResponse>";
    let result = parse_response(xml).unwrap();
    assert_eq!(result.as_str().unwrap(), "");
}

// `<array></array>` with no `<data>` tag is an empty array.
#[test]
fn parse_array_without_data_tag_is_empty() {
    let xml = r"<methodResponse><params><param><value><array></array></value></param></params></methodResponse>";
    let result = parse_response(xml).unwrap();
    assert!(result.as_array().unwrap().is_empty());
}

// `<params></params>` with no `<param>` child must produce a specific
// "empty params" error, not an EOF error.
#[test]
fn parse_empty_params_errors_with_specific_message() {
    let xml = r"<methodResponse><params></params></methodResponse>";
    let err = parse_response(xml).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("empty params"),
        "expected 'empty params' message, got: {msg}"
    );
}

// The methodResponse-finding loop must skip elements before
// <methodResponse> even if they look response-shaped.
#[test]
fn parse_response_skips_decoy_root_sibling() {
    let xml = r"<wrapper>
        <fakeResponse><params><param><value><string>decoy</string></value></param></params></fakeResponse>
        <methodResponse><params><param><value><string>real</string></value></param></params></methodResponse>
    </wrapper>";
    let result = parse_response(xml).unwrap();
    assert_eq!(result.as_str().unwrap(), "real");
}

// Inside <methodResponse>, the parser must skip non-fault/non-params
// elements rather than treating them as <params>.
#[test]
fn parse_response_skips_decoy_inside_method_response() {
    let xml = r"<methodResponse>
        <decoy><param><value><string>decoy</string></value></param></decoy>
        <params><param><value><string>real</string></value></param></params>
    </methodResponse>";
    let result = parse_response(xml).unwrap();
    assert_eq!(result.as_str().unwrap(), "real");
}

// Inside <params>, an unrelated start/end pair before <param> must not
// be treated as a param or as the end of params. The decoy carries its
// own value-shaped payload so the test fails closed if the param guard
// ever stops discriminating tag names.
#[test]
fn parse_first_param_skips_decoy_inside_params() {
    let xml = r"<methodResponse><params>
        <wrapper><value><string>decoy</string></value></wrapper>
        <param><value><string>real</string></value></param>
    </params></methodResponse>";
    let result = parse_response(xml).unwrap();
    assert_eq!(result.as_str().unwrap(), "real");
}

// Inside <param>, decoy elements before <value> must not be treated as
// the value.
#[test]
fn parse_value_skips_decoy_inside_param() {
    let xml = r"<methodResponse><params><param>
        <decoy>x</decoy>
        <value><string>real</string></value>
    </param></params></methodResponse>";
    let result = parse_response(xml).unwrap();
    assert_eq!(result.as_str().unwrap(), "real");
}

// Inside <array>, an unrelated start/end pair before <data> must not
// be treated as <data> or as the end of the array.
#[test]
fn parse_array_skips_decoy_before_data() {
    let xml = r"<methodResponse><params><param><value>
        <array>
            <wrapper><value><string>decoy</string></value></wrapper>
            <data><value><string>real</string></value></data>
        </array>
    </value></param></params></methodResponse>";
    let result = parse_response(xml).unwrap();
    let a = result.as_array().unwrap();
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].as_str().unwrap(), "real");
}

// Inside <data>, decoy elements between values must not be treated as
// values, and unrelated end tags must not terminate the data loop early.
#[test]
fn parse_array_skips_decoy_inside_data() {
    let xml = r"<methodResponse><params><param><value>
        <array><data>
            <wrapper></wrapper>
            <value><string>real</string></value>
        </data></array>
    </value></param></params></methodResponse>";
    let result = parse_response(xml).unwrap();
    let a = result.as_array().unwrap();
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].as_str().unwrap(), "real");
}

// A <decoy_member> looks structurally like a member but has the wrong
// tag name; the struct must produce no entries rather than parsing the
// decoy's contents as a member.
#[test]
fn parse_struct_skips_member_lookalike() {
    let xml = r"<methodResponse><params><param><value>
        <struct>
            <decoy_member><name>k</name><value><string>x</string></value></decoy_member>
        </struct>
    </value></param></params></methodResponse>";
    let result = parse_response(xml).unwrap();
    let s = result.as_struct().unwrap();
    assert!(s.is_empty(), "decoy_member should be skipped, got {s:?}");
}

// An unrelated end tag inside <struct> must not be treated as </struct>.
#[test]
fn parse_struct_does_not_terminate_on_unrelated_end_tag() {
    let xml = r"<methodResponse><params><param><value>
        <struct>
            <wrapper></wrapper>
            <member><name>k</name><value><string>v</string></value></member>
        </struct>
    </value></param></params></methodResponse>";
    let result = parse_response(xml).unwrap();
    let s = result.as_struct().unwrap();
    assert_eq!(s.get("k").unwrap().as_str().unwrap(), "v");
}

// An unrelated end tag inside <member> must not be treated as </member>.
#[test]
fn parse_member_does_not_terminate_on_unrelated_end_tag() {
    let xml = r"<methodResponse><params><param><value>
        <struct><member>
            <name>k</name>
            <wrapper></wrapper>
            <value><string>v</string></value>
        </member></struct>
    </value></param></params></methodResponse>";
    let result = parse_response(xml).unwrap();
    let s = result.as_struct().unwrap();
    assert_eq!(s.get("k").unwrap().as_str().unwrap(), "v");
}

// Unrelated end tags inside text content must not terminate
// read_text_content early.
#[test]
fn parse_string_continues_past_unrelated_end_tag() {
    let xml = r"<methodResponse><params><param><value>
        <string>before<wrapper></wrapper>after</string>
    </value></param></params></methodResponse>";
    let result = parse_response(xml).unwrap();
    let s = result.as_str().unwrap();
    assert!(
        s.contains("after"),
        "should keep reading past </wrapper>; got {s:?}"
    );
}
