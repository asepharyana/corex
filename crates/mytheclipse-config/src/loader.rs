//! Merges file and environment sources into a typed configuration value.

use std::marker::PhantomData;
use std::path::Path;

use serde_json::{Map, Value};

use crate::{Config, ConfigError};

/// Builds a typed configuration value from any number of sources.
///
/// Sources are merged in the order they're added; later sources override
/// earlier ones at the leaf level (objects are merged recursively, not
/// replaced wholesale). The conventional order is: defaults, then files
/// (base -> environment-specific), then environment variables (highest
/// priority, for secrets/overrides).
pub struct ConfigLoader<T> {
    value: Value,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Default for ConfigLoader<T> {
    fn default() -> Self {
        Self {
            value: Value::Object(Map::new()),
            _marker: PhantomData,
        }
    }
}

impl<T: Config> ConfigLoader<T> {
    /// Builds an empty loader.
    pub fn new() -> Self {
        Self::default()
    }

    /// Merges a JSON value directly (useful for defaults / tests).
    pub fn merge_value(mut self, value: Value) -> Self {
        deep_merge(&mut self.value, value);
        self
    }

    /// Reads `path`, parses it by extension (`.yaml`/`.yml`, `.json`,
    /// `.toml`), and merges the result.
    pub fn merge_file(mut self, path: &Path) -> Result<Self, ConfigError> {
        let contents = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::Io(format!("{}: {e}", path.display())))?;
        let parsed = parse_by_extension(path, &contents)?;
        deep_merge(&mut self.value, parsed);
        Ok(self)
    }

    /// Loads a `.env`-style file into the process environment (does not merge
    /// into the config value directly — call [`Self::merge_env`] afterward to
    /// pick the variables up).
    #[cfg(feature = "env")]
    pub fn load_dotenv(self, path: &Path) -> Result<Self, ConfigError> {
        dotenvy::from_path(path).map_err(|e| ConfigError::Io(e.to_string()))?;
        Ok(self)
    }

    /// Merges environment variables whose name starts with `prefix` (an
    /// underscore is inserted between the prefix and the field name if not
    /// already present). Nested fields use `__` as a separator, e.g.
    /// `APP_DATABASE__URL` maps to `{ "database": { "url": ... } }`.
    ///
    /// Values are parsed as JSON scalars when possible (`true`/`false`,
    /// integers, floats), otherwise kept as strings.
    pub fn merge_env(mut self, prefix: &str) -> Self {
        let collected = collect_env(prefix);
        deep_merge(&mut self.value, Value::Object(collected));
        self
    }

    /// Deserializes the merged value into `T`.
    pub fn build(self) -> Result<T, ConfigError> {
        serde_json::from_value(self.value).map_err(|e| ConfigError::Deserialize(e.to_string()))
    }

    /// Returns the current merged value without deserializing, for
    /// inspection/debugging.
    pub fn peek(&self) -> &Value {
        &self.value
    }
}

/// Parses `contents` according to `path`'s extension.
fn parse_by_extension(path: &Path, contents: &str) -> Result<Value, ConfigError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        #[cfg(feature = "yaml")]
        "yaml" | "yml" => serde_yaml::from_str(contents)
            .map_err(|e| ConfigError::Parse(e.to_string()))
            .map(yaml_to_json),
        "json" => serde_json::from_str(contents).map_err(|e| ConfigError::Parse(e.to_string())),
        #[cfg(feature = "toml")]
        "toml" => {
            let v: toml::Value =
                toml::from_str(contents).map_err(|e| ConfigError::Parse(e.to_string()))?;
            Ok(toml_to_json(v))
        }
        other => Err(ConfigError::UnsupportedFormat(format!(
            "no loader registered for `.{other}` (path: {})",
            path.display()
        ))),
    }
}

#[cfg(feature = "yaml")]
fn yaml_to_json(v: serde_yaml::Value) -> Value {
    // Round-trip through serde_json for a uniform merge representation.
    serde_json::to_value(v).unwrap_or(Value::Null)
}

#[cfg(feature = "toml")]
fn toml_to_json(v: toml::Value) -> Value {
    serde_json::to_value(v).unwrap_or(Value::Null)
}

/// Deep-merges `overlay` into `base`; scalars and arrays in `overlay` replace
/// `base`, objects are merged key-by-key recursively.
fn deep_merge(base: &mut Value, overlay: Value) {
    match (base, overlay) {
        (Value::Object(base_map), Value::Object(overlay_map)) => {
            for (k, v) in overlay_map {
                match base_map.get_mut(&k) {
                    Some(existing) => deep_merge(existing, v),
                    None => {
                        base_map.insert(k, v);
                    }
                }
            }
        }
        (base_slot, overlay_value) => {
            *base_slot = overlay_value;
        }
    }
}

/// Collects environment variables with `prefix` into a nested JSON object.
///
/// `PREFIX_FOO` -> `{"foo": ...}`; `PREFIX_FOO__BAR` -> `{"foo": {"bar": ...}}`.
fn collect_env(prefix: &str) -> Map<String, Value> {
    let mut root = Map::new();
    let full_prefix = if prefix.ends_with('_') {
        prefix.to_string()
    } else {
        format!("{prefix}_")
    };
    for (key, raw_value) in std::env::vars() {
        let Some(rest) = key.strip_prefix(&full_prefix) else {
            continue;
        };
        if rest.is_empty() {
            continue;
        }
        let path: Vec<String> = rest.split("__").map(|s| s.to_ascii_lowercase()).collect();
        insert_nested(&mut root, &path, coerce_scalar(&raw_value));
    }
    root
}

fn insert_nested(root: &mut Map<String, Value>, path: &[String], value: Value) {
    if path.len() == 1 {
        root.insert(path[0].clone(), value);
        return;
    }
    let entry = root
        .entry(path[0].clone())
        .or_insert_with(|| Value::Object(Map::new()));
    if let Value::Object(nested) = entry {
        insert_nested(nested, &path[1..], value);
    }
}

/// Parses `s` as a JSON scalar (`bool`/number) if possible, else keeps it as
/// a string.
fn coerce_scalar(s: &str) -> Value {
    if let Ok(b) = s.parse::<bool>() {
        return Value::Bool(b);
    }
    if let Ok(i) = s.parse::<i64>() {
        return Value::Number(i.into());
    }
    if let Ok(f) = s.parse::<f64>() {
        if let Some(n) = serde_json::Number::from_f64(f) {
            return Value::Number(n);
        }
    }
    Value::String(s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq)]
    struct Db {
        url: String,
        pool_size: u32,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct AppConfig {
        port: u16,
        debug: bool,
        database: Db,
    }

    #[test]
    fn merge_value_builds_typed_struct() {
        let value = serde_json::json!({
            "port": 8080,
            "debug": true,
            "database": { "url": "postgres://x", "pool_size": 10 }
        });
        let cfg: AppConfig = ConfigLoader::new().merge_value(value).build().unwrap();
        assert_eq!(
            cfg,
            AppConfig {
                port: 8080,
                debug: true,
                database: Db {
                    url: "postgres://x".into(),
                    pool_size: 10
                }
            }
        );
    }

    #[test]
    fn deep_merge_overrides_nested_leaf_only() {
        let base = serde_json::json!({ "port": 8080, "debug": false, "database": { "url": "a", "pool_size": 5 } });
        let overlay = serde_json::json!({ "database": { "pool_size": 20 } });
        let cfg: AppConfig = ConfigLoader::new()
            .merge_value(base)
            .merge_value(overlay)
            .build()
            .unwrap();
        assert_eq!(cfg.database.pool_size, 20);
        assert_eq!(cfg.database.url, "a"); // untouched sibling field preserved
        assert_eq!(cfg.port, 8080);
    }

    #[test]
    fn env_merge_maps_nested_and_types() {
        // Safe because this test owns unique env var names.
        std::env::set_var("CFGTEST_PORT", "9090");
        std::env::set_var("CFGTEST_DEBUG", "true");
        std::env::set_var("CFGTEST_DATABASE__URL", "redis://y");
        std::env::set_var("CFGTEST_DATABASE__POOL_SIZE", "42");

        let cfg: AppConfig = ConfigLoader::new().merge_env("CFGTEST").build().unwrap();
        assert_eq!(cfg.port, 9090);
        assert!(cfg.debug);
        assert_eq!(cfg.database.url, "redis://y");
        assert_eq!(cfg.database.pool_size, 42);

        std::env::remove_var("CFGTEST_PORT");
        std::env::remove_var("CFGTEST_DEBUG");
        std::env::remove_var("CFGTEST_DATABASE__URL");
        std::env::remove_var("CFGTEST_DATABASE__POOL_SIZE");
    }

    #[test]
    fn env_overrides_file_value() {
        std::env::set_var("CFGTEST2_PORT", "7000");
        let base = serde_json::json!({ "port": 1, "debug": false, "database": { "url": "a", "pool_size": 1 } });
        let cfg: AppConfig = ConfigLoader::new()
            .merge_value(base)
            .merge_env("CFGTEST2")
            .build()
            .unwrap();
        assert_eq!(cfg.port, 7000);
        std::env::remove_var("CFGTEST2_PORT");
    }

    #[cfg(feature = "yaml")]
    #[test]
    fn merge_yaml_file() {
        use std::io::Write;
        let mut file = tempfile::Builder::new().suffix(".yaml").tempfile().unwrap();
        writeln!(
            file,
            "port: 3000\ndebug: false\ndatabase:\n  url: sqlite://mem\n  pool_size: 3\n"
        )
        .unwrap();
        let cfg: AppConfig = ConfigLoader::new()
            .merge_file(file.path())
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(cfg.port, 3000);
        assert_eq!(cfg.database.url, "sqlite://mem");
    }

    #[test]
    fn merge_json_file() {
        use std::io::Write;
        let mut file = tempfile::Builder::new().suffix(".json").tempfile().unwrap();
        write!(
            file,
            r#"{{"port": 4000, "debug": true, "database": {{"url": "mysql://x", "pool_size": 7}}}}"#
        )
        .unwrap();
        let cfg: AppConfig = ConfigLoader::new()
            .merge_file(file.path())
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(cfg.port, 4000);
        assert_eq!(cfg.database.pool_size, 7);
    }

    #[cfg(feature = "toml")]
    #[test]
    fn merge_toml_file() {
        use std::io::Write;
        let mut file = tempfile::Builder::new().suffix(".toml").tempfile().unwrap();
        writeln!(
            file,
            "port = 5000\ndebug = false\n\n[database]\nurl = \"file://local\"\npool_size = 2\n"
        )
        .unwrap();
        let cfg: AppConfig = ConfigLoader::new()
            .merge_file(file.path())
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(cfg.port, 5000);
        assert_eq!(cfg.database.pool_size, 2);
    }

    #[test]
    fn unsupported_extension_errors() {
        let file = tempfile::Builder::new().suffix(".ini").tempfile().unwrap();
        std::fs::write(file.path(), "unused").unwrap();
        let result: Result<AppConfig, _> = ConfigLoader::new()
            .merge_file(file.path())
            .and_then(|l| l.build());
        assert!(matches!(result, Err(ConfigError::UnsupportedFormat(_))));
    }
}
