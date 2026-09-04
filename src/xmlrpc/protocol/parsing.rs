//! XML-RPC response parsing.

use std::collections::BTreeMap;

use quick_xml::escape::resolve_predefined_entity;
use quick_xml::events::{BytesRef, Event};
use quick_xml::Reader;

use crate::error::{BzrError, Result};
use crate::xmlrpc::protocol::fault::fault_to_error;
use crate::xmlrpc::protocol::Value;

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

/// Resolve an entity reference event (`&name;` or `&#decimal;`/`&#xhex;`)
/// to its replacement text.
///
/// quick-xml 0.42 emits entity references as separate `Event::GeneralRef`
/// events instead of leaving them inside text events, so text content no
/// longer contains escapes to unescape.
fn resolve_entity_ref(reference: &BytesRef) -> Result<String> {
    if reference.is_char_ref() {
        let ch = reference
            .resolve_char_ref()
            .map_err(|err| BzrError::XmlRpc(format!("XML character reference error: {err}")))?
            .ok_or_else(|| BzrError::XmlRpc("empty character reference".into()))?;
        Ok(ch.to_string())
    } else {
        resolve_predefined_entity(reference)
            .map(str::to_owned)
            .ok_or_else(|| BzrError::XmlRpc(format!("unknown entity '&{};'", reference.as_ref())))
    }
}

/// Parse an XML-RPC method response body.
///
/// Returns the first `<param>` value on success, or maps a fault response
/// to `BzrError::XmlRpc`.
pub fn parse_response(xml: &str) -> Result<Value> {
    let mut reader = Reader::from_str(xml);
    // Trimming is done by `read_text_content` instead: the reader's own
    // trim_text(true) would strip whitespace around `Event::GeneralRef`
    // events, which quick-xml 0.42 emits for every `&...;` entity, corrupting
    // text such as `Tom &amp; Jerry` into `Tom&Jerry`.
    reader.config_mut().trim_text(false);
    // Emit self-closing tags (`<value/>`, `<struct/>`, `<array/>`, which
    // Bugzilla returns for empty/null fields) as a Start+End pair so the
    // Start/End-based parsers below handle them uniformly.
    reader.config_mut().expand_empty_elements = true;

    loop {
        match next_event(&mut reader, "looking for methodResponse")? {
            Event::Start(ref e) if e.name().as_ref() == "methodResponse" => break,
            _ => {}
        }
    }

    loop {
        match next_event(&mut reader, "in methodResponse")? {
            Event::Start(ref e) if e.name().as_ref() == "fault" => {
                let value = parse_value(&mut reader)?;
                return Err(fault_to_error(&value));
            }
            Event::Start(ref e) if e.name().as_ref() == "params" => {
                return parse_first_param(&mut reader);
            }
            _ => {}
        }
    }
}

fn parse_first_param(reader: &mut Reader<&[u8]>) -> Result<Value> {
    loop {
        match next_event(reader, "in params")? {
            Event::Start(ref e) if e.name().as_ref() == "param" => {
                return parse_value(reader);
            }
            Event::End(ref e) if e.name().as_ref() == "params" => {
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
            Event::Start(ref e) if e.name().as_ref() == "value" => break,
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
fn is_value_end(name: &str) -> bool {
    name == "value"
}

/// Parse the content inside a `<value>` element (after the opening tag).
fn parse_value_content(reader: &mut Reader<&[u8]>) -> Result<Value> {
    loop {
        match next_event(reader, "in value")? {
            Event::Start(ref e) => {
                let tag = e.name();
                let tag_name = tag.as_ref();
                let value = match tag_name {
                    "string" => Value::String(read_text_content(reader, "string", None)?),
                    "int" | "i4" => {
                        let text = read_text_content(reader, tag_name, None)?;
                        let n = text.parse::<i64>().map_err(|e| {
                            BzrError::XmlRpc(format!("invalid integer '{text}': {e}"))
                        })?;
                        Value::Int(n)
                    }
                    "boolean" => {
                        let text = read_text_content(reader, "boolean", None)?;
                        Value::Bool(text == "1" || text.eq_ignore_ascii_case("true"))
                    }
                    "double" => {
                        let text = read_text_content(reader, "double", None)?;
                        let d = text.parse::<f64>().map_err(|e| {
                            BzrError::XmlRpc(format!("invalid double '{text}': {e}"))
                        })?;
                        Value::Double(d)
                    }
                    "dateTime.iso8601" => {
                        Value::DateTime(read_text_content(reader, "dateTime.iso8601", None)?)
                    }
                    "base64" => {
                        let text = read_text_content(reader, "base64", None)?;
                        let bytes = base64::Engine::decode(
                            &base64::engine::general_purpose::STANDARD,
                            &text,
                        )
                        .map_err(|e| BzrError::XmlRpc(format!("invalid base64: {e}")))?;
                        Value::Base64(bytes)
                    }
                    "array" => parse_array(reader)?,
                    "struct" => parse_struct(reader)?,
                    other => {
                        return Err(BzrError::XmlRpc(format!("unknown value type: {other}")));
                    }
                };
                // Read closing </value>
                skip_to_end(reader, "value")?;
                return Ok(value);
            }
            // Bare text inside <value> without a type tag → treat as string.
            // Entity references arrive as `Event::GeneralRef` events in
            // quick-xml 0.42; the already-consumed content event is seeded
            // into `read_text_content` as `Some(...)`. Whitespace-only text
            // is formatting (quick-xml 0.41's text trimming dropped it).
            Event::Text(ref e) => {
                if e.as_ref().trim().is_empty() {
                    continue;
                }
                let text =
                    read_text_content(reader, "value", Some((false, e.as_ref().to_string())))?;
                return Ok(Value::String(text));
            }
            Event::GeneralRef(ref e) => {
                let text =
                    read_text_content(reader, "value", Some((true, resolve_entity_ref(e)?)))?;
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

/// Accumulate the text content of the current element into a `String`,
/// stopping at `</{end_tag}>` (which is consumed).
///
/// `first` carries the first content event's piece when the caller has
/// already consumed it (bare `<value>` text): `Some((true, ...))` for an
/// entity reference replacement, `Some((false, ...))` for raw text.
///
/// Whitespace handling mirrors the trimming quick-xml 0.41 applied to each
/// text event: leading/trailing formatting whitespace at markup boundaries
/// is trimmed, but whitespace adjacent to an entity reference is content
/// and is preserved (0.42 splits `&...;` into separate `GeneralRef`
/// events, so the reader-level `trim_text(true)` would otherwise conjoin
/// `Tom &amp; Jerry` into `Tom&amp;Jerry`).
fn read_text_content(
    reader: &mut Reader<&[u8]>,
    end_tag: &str,
    first: Option<(bool, String)>,
) -> Result<String> {
    let mut text = String::new();
    // `pending` holds the most recent text piece. Its trailing whitespace
    // can only be trimmed once the next event is known: a `GeneralRef`
    // means the whitespace is content, while a markup boundary means it is
    // formatting.
    let mut pending: Option<String>;
    // True when nothing has been emitted since the last markup boundary;
    // leading whitespace of the next text piece is then formatting and
    // trimmed.
    let mut at_start;
    match first {
        Some((true, piece)) => {
            text.push_str(&piece);
            pending = None;
            at_start = false;
        }
        Some((false, piece)) => {
            pending = Some(piece.trim_start().to_string());
            at_start = false;
        }
        None => {
            pending = None;
            at_start = true;
        }
    }
    let context = format!("reading <{end_tag}>");
    loop {
        match next_event(reader, &context)? {
            Event::Text(ref e) => {
                let piece = if at_start {
                    e.as_ref().trim_start()
                } else {
                    e.as_ref()
                };
                if let Some(p) = pending.take() {
                    text.push_str(&p);
                }
                pending = Some(piece.to_string());
                at_start = false;
            }
            Event::GeneralRef(ref e) => {
                if let Some(p) = pending.take() {
                    // A ref follows, so the pending text's trailing
                    // whitespace is content; keep it intact.
                    text.push_str(&p);
                }
                text.push_str(&resolve_entity_ref(e)?);
                at_start = false;
            }
            Event::CData(ref e) => {
                if let Some(p) = pending.take() {
                    text.push_str(p.trim_end());
                }
                text.push_str(e.as_ref());
                at_start = true;
            }
            Event::End(ref e) if e.name().as_ref() == end_tag => {
                if let Some(p) = pending.take() {
                    text.push_str(p.trim_end());
                }
                return Ok(text);
            }
            // Any other markup is a boundary for both trims.
            _ => {
                if let Some(p) = pending.take() {
                    text.push_str(p.trim_end());
                }
                at_start = true;
            }
        }
    }
}

fn parse_array(reader: &mut Reader<&[u8]>) -> Result<Value> {
    // Expect <data>, then values, then </data>, then </array>
    let mut items = Vec::new();

    // Find <data>
    loop {
        match next_event(reader, "in array")? {
            Event::Start(ref e) if e.name().as_ref() == "data" => break,
            Event::End(ref e) if e.name().as_ref() == "array" => {
                return Ok(Value::Array(items));
            }
            _ => {}
        }
    }

    // Read values until </data>
    loop {
        match next_event(reader, "in array data")? {
            Event::Start(ref e) if e.name().as_ref() == "value" => {
                items.push(parse_value_content(reader)?);
            }
            Event::End(ref e) if e.name().as_ref() == "data" => break,
            _ => {}
        }
    }

    // Read closing </array>
    skip_to_end(reader, "array")?;
    Ok(Value::Array(items))
}

fn parse_struct(reader: &mut Reader<&[u8]>) -> Result<Value> {
    let mut members = BTreeMap::new();

    loop {
        match next_event(reader, "in struct")? {
            Event::Start(ref e) if e.name().as_ref() == "member" => {
                let (name, value) = parse_member(reader)?;
                members.insert(name, value);
            }
            Event::End(ref e) if e.name().as_ref() == "struct" => {
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
                if tag.as_ref() == "name" {
                    name = Some(read_text_content(reader, "name", None)?);
                } else if tag.as_ref() == "value" {
                    value = Some(parse_value_content(reader)?);
                }
            }
            Event::End(ref e) if e.name().as_ref() == "member" => {
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

fn skip_to_end(reader: &mut Reader<&[u8]>, tag: &str) -> Result<()> {
    let mut depth: u32 = 1;
    let context = format!("skipping to </{tag}>");
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
