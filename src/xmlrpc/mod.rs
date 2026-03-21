//! XML-RPC transport adapter for Bugzilla servers that use the XML-RPC API
//! instead of (or alongside) the REST API. Used internally by `BugzillaClient`
//! when the detected `ApiMode` is `XmlRpc` or `Hybrid`.

pub(crate) mod client;

use std::collections::BTreeMap;

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::error::{BzrError, Result};

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

/// XML-RPC value types used to build requests and parse responses.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    String(String),
    Int(i64),
    Bool(bool),
    Double(f64),
    DateTime(String),
    Base64(Vec<u8>),
    Array(Vec<Value>),
    Struct(BTreeMap<String, Value>),
}

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(n) => Some(*n),
            _ => None,
        }
    }

    #[expect(
        dead_code,
        reason = "API completeness — all Value variants should be accessible"
    )]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_struct(&self) -> Option<&BTreeMap<String, Value>> {
        match self {
            Value::Struct(m) => Some(m),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    #[expect(
        dead_code,
        reason = "API completeness — all Value variants should be accessible"
    )]
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Double(d) => Some(*d),
            _ => None,
        }
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::String(s.to_string())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::String(s)
    }
}

impl From<i64> for Value {
    fn from(n: i64) -> Self {
        Value::Int(n)
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

impl From<Vec<Value>> for Value {
    fn from(v: Vec<Value>) -> Self {
        Value::Array(v)
    }
}

impl From<BTreeMap<String, Value>> for Value {
    fn from(m: BTreeMap<String, Value>) -> Self {
        Value::Struct(m)
    }
}

/// Escape XML special characters in text content.
fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
    out
}

fn write_value(buf: &mut String, value: &Value) {
    buf.push_str("<value>");
    match value {
        Value::String(s) => {
            buf.push_str("<string>");
            buf.push_str(&xml_escape(s));
            buf.push_str("</string>");
        }
        Value::Int(n) => {
            buf.push_str("<int>");
            buf.push_str(&n.to_string());
            buf.push_str("</int>");
        }
        Value::Bool(b) => {
            buf.push_str("<boolean>");
            buf.push(if *b { '1' } else { '0' });
            buf.push_str("</boolean>");
        }
        Value::Double(d) => {
            buf.push_str("<double>");
            buf.push_str(&d.to_string());
            buf.push_str("</double>");
        }
        Value::DateTime(s) => {
            buf.push_str("<dateTime.iso8601>");
            buf.push_str(&xml_escape(s));
            buf.push_str("</dateTime.iso8601>");
        }
        Value::Base64(data) => {
            buf.push_str("<base64>");
            buf.push_str(&base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                data,
            ));
            buf.push_str("</base64>");
        }
        Value::Array(items) => {
            buf.push_str("<array><data>");
            for item in items {
                write_value(buf, item);
            }
            buf.push_str("</data></array>");
        }
        Value::Struct(members) => {
            buf.push_str("<struct>");
            for (name, val) in members {
                buf.push_str("<member><name>");
                buf.push_str(&xml_escape(name));
                buf.push_str("</name>");
                write_value(buf, val);
                buf.push_str("</member>");
            }
            buf.push_str("</struct>");
        }
    }
    buf.push_str("</value>");
}

/// Build an XML-RPC method call body.
///
/// The `params` map is sent as a single struct parameter (Bugzilla convention).
pub fn build_request(method: &str, params: BTreeMap<String, Value>) -> String {
    let mut buf = String::with_capacity(512);
    buf.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
    buf.push_str("<methodCall><methodName>");
    buf.push_str(&xml_escape(method));
    buf.push_str("</methodName><params><param>");
    write_value(&mut buf, &Value::Struct(params));
    buf.push_str("</param></params></methodCall>");
    buf
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
                let text = e
                    .unescape()
                    .map_err(|err| BzrError::XmlRpc(format!("XML unescape error: {err}")))?
                    .into_owned();
                skip_to_end(reader, b"value")?;
                return Ok(Value::String(text));
            }
            // Empty <value/> → empty string
            Event::End(ref e) if e.name().as_ref() == b"value" => {
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
                text.push_str(
                    &e.unescape()
                        .map_err(|err| BzrError::XmlRpc(format!("XML unescape error: {err}")))?,
                );
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

fn fault_to_error(value: &Value) -> BzrError {
    if let Some(members) = value.as_struct() {
        let code = members
            .get("faultCode")
            .and_then(Value::as_i64)
            .unwrap_or(-1);
        let msg = members
            .get("faultString")
            .and_then(Value::as_str)
            .unwrap_or("unknown XML-RPC fault");
        BzrError::XmlRpc(format!("fault {code}: {msg}"))
    } else {
        BzrError::XmlRpc("malformed fault response".into())
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn build_simple_request() {
        let mut params = BTreeMap::new();
        params.insert("product".into(), Value::from("TestProduct"));
        params.insert("limit".into(), Value::Int(10));

        let xml = build_request("Bug.search", params);

        assert!(xml.contains("<methodName>Bug.search</methodName>"));
        assert!(xml.contains("<name>limit</name><value><int>10</int></value>"));
        assert!(xml.contains("<name>product</name><value><string>TestProduct</string></value>"));
    }

    #[test]
    fn build_request_with_bool_and_array() {
        let mut params = BTreeMap::new();
        params.insert("active".into(), Value::Bool(true));
        params.insert(
            "ids".into(),
            Value::Array(vec![Value::Int(1), Value::Int(2)]),
        );

        let xml = build_request("Bug.get", params);

        assert!(xml.contains("<boolean>1</boolean>"));
        assert!(xml.contains(
            "<array><data><value><int>1</int></value><value><int>2</int></value></data></array>"
        ));
    }

    #[test]
    fn build_request_escapes_special_chars() {
        let mut params = BTreeMap::new();
        params.insert("query".into(), Value::from("foo & bar <baz>"));

        let xml = build_request("Test.method", params);

        assert!(xml.contains("foo &amp; bar &lt;baz&gt;"));
    }

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

    #[test]
    fn roundtrip_nested_struct() {
        let mut inner = BTreeMap::new();
        inner.insert("key".into(), Value::from("val"));
        let mut params = BTreeMap::new();
        params.insert("nested".into(), Value::Struct(inner));

        let xml = build_request("Test", params);
        assert!(xml.contains("<name>nested</name>"));
        assert!(xml.contains("<name>key</name><value><string>val</string></value>"));
    }

    #[test]
    fn value_conversions() {
        assert_eq!(Value::from("hello").as_str().unwrap(), "hello");
        assert_eq!(Value::from(42i64).as_i64().unwrap(), 42);
        assert!(Value::from(true).as_bool().unwrap());

        let s = Value::String("test".into());
        assert!(s.as_i64().is_none());
        assert!(s.as_bool().is_none());
        assert!(s.as_struct().is_none());
        assert!(s.as_array().is_none());
        assert!(s.as_f64().is_none());
    }
}
