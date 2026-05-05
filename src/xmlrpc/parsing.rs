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
#[path = "parsing_tests.rs"]
mod tests;
