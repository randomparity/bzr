//! XML-RPC request body construction.

use std::collections::BTreeMap;

use crate::xmlrpc::protocol::Value;

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

fn render_xmlrpc_value(buf: &mut String, value: &Value) {
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
                render_xmlrpc_value(buf, item);
            }
            buf.push_str("</data></array>");
        }
        Value::Struct(members) => {
            buf.push_str("<struct>");
            for (name, val) in members {
                buf.push_str("<member><name>");
                buf.push_str(&xml_escape(name));
                buf.push_str("</name>");
                render_xmlrpc_value(buf, val);
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
    render_xmlrpc_value(&mut buf, &Value::Struct(params));
    buf.push_str("</param></params></methodCall>");
    buf
}

#[cfg(test)]
#[path = "call_tests.rs"]
mod tests;
