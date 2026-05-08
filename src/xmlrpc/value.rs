use std::collections::BTreeMap;

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

    #[cfg(test)]
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
