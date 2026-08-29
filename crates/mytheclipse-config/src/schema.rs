//! JSON Schema generation for config types (feature `schema`).
//!
//! Generate JSON Schema from your config struct — useful for:
//! - Runtime validation
//! - Documentation / auto-generated config UIs
//! - Editor autocomplete via schema-store.json
//!
//! ```ignore
//! use serde::Deserialize;
//! use mytheclipse_config::schema::ConfigSchema;
//!
//! #[derive(Debug, Deserialize, Default)]
//! struct AppConfig {
//!     port: u16,
//! }
//!
//! let schema = ConfigSchema::generate::<AppConfig>();
//! println!("schema type: {}", schema.r#type);
//! ```

use serde_json::Value;
use std::collections::BTreeMap;

/// A minimal JSON Schema for documentation and validation.
#[derive(Debug, Clone)]
pub struct ConfigSchema {
    pub r#type: String,
    pub properties: BTreeMap<String, PropertySchema>,
    pub required: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PropertySchema {
    pub r#type: String,
    pub description: Option<String>,
    pub default: Option<Value>,
    pub properties: Option<BTreeMap<String, PropertySchema>>,
    pub required: Option<Vec<String>>,
}

impl ConfigSchema {
    /// Generates a schema for the given type (requires serde derive support).
    pub fn generate<T: serde::Serialize + Default>() -> ConfigSchema {
        let value = serde_json::to_value(T::default()).unwrap_or(Value::Null);
        let mut properties = BTreeMap::new();
        let mut required = Vec::new();

        if let Value::Object(map) = &value {
            for (k, v) in map {
                properties.insert(
                    k.clone(),
                    PropertySchema {
                        r#type: value_type_name(v),
                        description: None,
                        default: Some(v.clone()),
                        properties: None,
                        required: None,
                    },
                );
                required.push(k.clone());
            }
        }

        ConfigSchema {
            r#type: "object".to_string(),
            properties,
            required,
        }
    }
}

fn value_type_name(v: &Value) -> String {
    match v {
        Value::Null => "null".to_string(),
        Value::Bool(_) => "boolean".to_string(),
        Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                "integer".to_string()
            } else if n.is_f64() {
                "number".to_string()
            } else {
                "string".to_string()
            }
        }
        Value::String(_) => "string".to_string(),
        Value::Array(_) => "array".to_string(),
        Value::Object(_) => "object".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    #[derive(Serialize, Default)]
    struct TestConfig {
        port: u16,
    }

    #[test]
    fn generates_schema() {
        let schema = ConfigSchema::generate::<TestConfig>();
        assert_eq!(schema.r#type, "object");
        assert!(schema.properties.contains_key("port"));
    }
}
