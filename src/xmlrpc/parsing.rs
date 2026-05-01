//! XML-RPC response parsing.

use std::collections::BTreeMap;

use quick_xml::escape::unescape;
use quick_xml::events::{BytesText, Event};
use quick_xml::Reader;

use crate::error::{BzrError, Result};
use crate::xmlrpc::fault::fault_to_error;
use crate::xmlrpc::Value;

/// Convert an XML parse error to a `BzrError::XmlRpc`.
fn xml_parse_err(e: &quick_xml::Error) -> BzrError {
    BzrError::XmlRpc(format!("XML parse error: {e}"))
}

/// Return an `Err(BzrError::XmlRpc)` for unexpected EOF with context.
fn unexpected_eof(context: &str) -> BzrError {
    BzrError::XmlRpc(format!("unexpected EOF {context}"))
}

/// Read the next XML event, converting parse errors and EOF to `BzrError`.
fn next_event<'a>(reader: &mut Reader<&'a [u8]>, context: &str) -> Result<Event<'a>> {
    match reader.read_event() {
        Ok(Event::Eof) => Err(unexpected_eof(context)),
        Err(e) => Err(xml_parse_err(&e)),
        Ok(event) => Ok(event),
    }
}

/// Decode and unescape XML text content.
fn decode_text(text: &BytesText<'_>) -> Result<String> {
    let decoded = text
        .decode()
        .map_err(|err| BzrError::XmlRpc(format!("XML decode error: {err}")))?;
    let unescaped =
        unescape(&decoded).map_err(|err| BzrError::XmlRpc(format!("XML unescape error: {err}")))?;
    Ok(unescaped.into_owned())
}

/// Parse an XML-RPC method response body.
///
/// Returns the first `<param>` value on success, or maps a fault response
/// to `BzrError::XmlRpc`.
pub fn parse_response(xml: &str) -> Result<Value> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    loop {
        match next_event(&mut reader, "looking for methodResponse")? {
            Event::Start(ref e) if e.name().as_ref() == b"methodResponse" => break,
            _ => {}
        }
    }

    loop {
        match next_event(&mut reader, "in methodResponse")? {
            Event::Start(ref e) if e.name().as_ref() == b"fault" => {
                let value = parse_value(&mut reader)?;
                return Err(fault_to_error(&value));
            }
            Event::Start(ref e) if e.name().as_ref() == b"params" => {
                return parse_first_param(&mut reader);
            }
            _ => {}
        }
    }
}

fn parse_first_param(reader: &mut Reader<&[u8]>) -> Result<Value> {
    loop {
        match next_event(reader, "in params")? {
            Event::Start(ref e) if e.name().as_ref() == b"param" => {
                return parse_value(reader);
            }
            Event::End(ref e) if e.name().as_ref() == b"params" => {
                return Err(BzrError::XmlRpc("empty params in response".into()));
            }
            _ => {}
        }
    }
}

/// Parse a `<value>` element. Advances the reader past the closing `</value>`.
fn parse_value(reader: &mut Reader<&[u8]>) -> Result<Value> {
    loop {
        match next_event(reader, "looking for value")? {
            Event::Start(ref e) if e.name().as_ref() == b"value" => break,
            _ => {}
        }
    }

    parse_value_content(reader)
}

/// True iff `name` is the `</value>` end-tag.
///
/// Extracted so the always-true / inverted-`==` mutations on the empty-value
/// detection arm in `parse_value_content` are referenced by a stable name in
/// `.cargo/mutants.toml`. The `with false` mutation IS catchable (and remains
/// in the test set); only the equivalent ones are skipped.
fn is_value_end(name: &[u8]) -> bool {
    name == b"value"
}

/// Parse the content inside a `<value>` element (after the opening tag).
fn parse_value_content(reader: &mut Reader<&[u8]>) -> Result<Value> {
    loop {
        match next_event(reader, "in value")? {
            Event::Start(ref e) => {
                let tag = e.name();
                let tag_bytes = tag.as_ref();
                let value = match tag_bytes {
                    b"string" => Value::String(read_text_content(reader, b"string")?),
                    b"int" | b"i4" => {
                        let text = read_text_content(reader, tag_bytes)?;
                        let n = text.parse::<i64>().map_err(|e| {
                            BzrError::XmlRpc(format!("invalid integer '{text}': {e}"))
                        })?;
                        Value::Int(n)
                    }
                    b"boolean" => {
                        let text = read_text_content(reader, b"boolean")?;
                        Value::Bool(text == "1" || text.eq_ignore_ascii_case("true"))
                    }
                    b"double" => {
                        let text = read_text_content(reader, b"double")?;
                        let d = text.parse::<f64>().map_err(|e| {
                            BzrError::XmlRpc(format!("invalid double '{text}': {e}"))
                        })?;
                        Value::Double(d)
                    }
                    b"dateTime.iso8601" => {
                        Value::DateTime(read_text_content(reader, b"dateTime.iso8601")?)
                    }
                    b"base64" => {
                        let text = read_text_content(reader, b"base64")?;
                        let bytes = base64::Engine::decode(
                            &base64::engine::general_purpose::STANDARD,
                            &text,
                        )
                        .map_err(|e| BzrError::XmlRpc(format!("invalid base64: {e}")))?;
                        Value::Base64(bytes)
                    }
                    b"array" => parse_array(reader)?,
                    b"struct" => parse_struct(reader)?,
                    other => {
                        let name = String::from_utf8_lossy(other);
                        return Err(BzrError::XmlRpc(format!("unknown value type: {name}")));
                    }
                };
                // Read closing </value>
                skip_to_end(reader, b"value")?;
                return Ok(value);
            }
            // Bare text inside <value> without a type tag → treat as string
            Event::Text(ref e) => {
                let text = decode_text(e)?;
                skip_to_end(reader, b"value")?;
                return Ok(Value::String(text));
            }
            // Empty `<value></value>` → empty string.
            Event::End(ref e) if is_value_end(e.name().as_ref()) => {
                return Ok(Value::String(String::new()));
            }
            _ => {}
        }
    }
}

fn read_text_content(reader: &mut Reader<&[u8]>, end_tag: &[u8]) -> Result<String> {
    let mut text = String::new();
    let context = format!("reading <{}>", String::from_utf8_lossy(end_tag));
    loop {
        match next_event(reader, &context)? {
            Event::Text(ref e) => {
                text.push_str(&decode_text(e)?);
            }
            Event::CData(ref e) => {
                text.push_str(
                    std::str::from_utf8(e.as_ref())
                        .map_err(|e| BzrError::XmlRpc(format!("invalid UTF-8 in CDATA: {e}")))?,
                );
            }
            Event::End(ref e) if e.name().as_ref() == end_tag => {
                return Ok(text);
            }
            _ => {}
        }
    }
}

fn parse_array(reader: &mut Reader<&[u8]>) -> Result<Value> {
    // Expect <data>, then values, then </data>, then </array>
    let mut items = Vec::new();

    // Find <data>
    loop {
        match next_event(reader, "in array")? {
            Event::Start(ref e) if e.name().as_ref() == b"data" => break,
            Event::End(ref e) if e.name().as_ref() == b"array" => {
                return Ok(Value::Array(items));
            }
            _ => {}
        }
    }

    // Read values until </data>
    loop {
        match next_event(reader, "in array data")? {
            Event::Start(ref e) if e.name().as_ref() == b"value" => {
                items.push(parse_value_content(reader)?);
            }
            Event::End(ref e) if e.name().as_ref() == b"data" => break,
            _ => {}
        }
    }

    // Read closing </array>
    skip_to_end(reader, b"array")?;
    Ok(Value::Array(items))
}

fn parse_struct(reader: &mut Reader<&[u8]>) -> Result<Value> {
    let mut members = BTreeMap::new();

    loop {
        match next_event(reader, "in struct")? {
            Event::Start(ref e) if e.name().as_ref() == b"member" => {
                let (name, value) = parse_member(reader)?;
                members.insert(name, value);
            }
            Event::End(ref e) if e.name().as_ref() == b"struct" => {
                return Ok(Value::Struct(members));
            }
            _ => {}
        }
    }
}

fn parse_member(reader: &mut Reader<&[u8]>) -> Result<(String, Value)> {
    let mut name = None;
    let mut value = None;

    loop {
        match next_event(reader, "in member")? {
            Event::Start(ref e) => {
                let tag = e.name();
                if tag.as_ref() == b"name" {
                    name = Some(read_text_content(reader, b"name")?);
                } else if tag.as_ref() == b"value" {
                    value = Some(parse_value_content(reader)?);
                }
            }
            Event::End(ref e) if e.name().as_ref() == b"member" => {
                let n =
                    name.ok_or_else(|| BzrError::XmlRpc("struct member missing name".into()))?;
                let v = value.ok_or_else(|| {
                    BzrError::XmlRpc(format!("struct member '{n}' missing value"))
                })?;
                return Ok((n, v));
            }
            _ => {}
        }
    }
}

fn skip_to_end(reader: &mut Reader<&[u8]>, tag: &[u8]) -> Result<()> {
    let mut depth: u32 = 1;
    let context = format!("skipping to </{}>", String::from_utf8_lossy(tag));
    loop {
        match next_event(reader, &context)? {
            Event::Start(ref e) if e.name().as_ref() == tag => depth += 1,
            Event::End(ref e) if e.name().as_ref() == tag => {
                depth -= 1;
                if depth == 0 {
                    return Ok(());
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;

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
        let xml =
            r"<methodResponse><params><param><value></value></param></params></methodResponse>";
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
}
