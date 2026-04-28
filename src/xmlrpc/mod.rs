//! XML-RPC transport adapter for Bugzilla servers that use the XML-RPC API
//! instead of (or alongside) the REST API. Used internally by `BugzillaClient`
//! when the detected `ApiMode` is `XmlRpc` or `Hybrid`.

mod call;
pub(crate) mod client;
mod fault;
mod parsing;

use std::collections::BTreeMap;

pub use call::build_request;
pub use parsing::parse_response;

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

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    #[cfg(test)]
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

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;

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

    #[test]
    fn from_string_value() {
        let v: Value = "owned".to_string().into();
        assert_eq!(v.as_str().unwrap(), "owned");
    }

    #[test]
    fn from_vec_and_btreemap_value() {
        let arr: Value = vec![Value::Int(1), Value::Int(2)].into();
        assert_eq!(arr.as_array().unwrap().len(), 2);

        let mut m = BTreeMap::new();
        m.insert("k".into(), Value::from("v"));
        let s: Value = m.into();
        assert_eq!(s.as_struct().unwrap().get("k").unwrap().as_str(), Some("v"));
    }
}
