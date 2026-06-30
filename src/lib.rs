#[allow(unused_imports)]
use if_chain::if_chain;
use notify::{
    event::{DataChange, ModifyKind},
    Error, Event, EventKind, RecommendedWatcher, Watcher,
};
use serde::de::DeserializeOwned;
#[cfg(feature = "xml")]
use serde_xml_rs as serde_xml;
#[cfg(any(
    feature = "json",
    feature = "yaml",
    feature = "toml",
    feature = "xml",
    feature = "ini"
))]
use std::fs;
use std::{
    any::Any,
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        mpsc::{channel, Receiver},
        Arc, RwLock,
    },
    time::Duration,
};
#[cfg(feature = "toml")]
use toml as serde_toml;

/// Represents the format of the configuration file.
#[derive(Clone, Debug)]
pub enum Format {
    #[cfg(feature = "json")]
    Json,
    #[cfg(feature = "yaml")]
    Yaml,
    #[cfg(feature = "toml")]
    Toml,
    #[cfg(feature = "xml")]
    Xml,
    #[cfg(feature = "ini")]
    Ini,
}

/// Represents the identifier of a configuration.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
pub struct ConfigId(String);
impl ConfigId {
    /// Creates a new `ConfigId` with the given identifier.
    pub fn new<S: Into<String>>(id: S) -> Self {
        Self(id.into())
    }
}

struct Config {
    value: Box<dyn Any + Send + Sync>,
    _watcher: RecommendedWatcher,
}

/// Represents a collection of reloadable configurations.
#[derive(Clone)]
pub struct Reloadify(Arc<RwLock<HashMap<ConfigId, Config>>>);

/// Represents an error that can occur in the `Reloadify` struct.
#[derive(Debug, thiserror::Error)]
pub enum ReloadifyError {
    #[error("Failed to acquire lock")]
    GetLockError,
    #[error("Failed to load config: {0}")]
    LoadConfigError(#[from] std::io::Error),
    #[error("Failed to deserialize config: {0}")]
    DeserializeError(String),
    #[error("Failed to watch: {0}")]
    WatchError(#[from] notify::Error),
    #[error("Failed to downcast")]
    DowncastError,
    #[error("Config does not exist")]
    ConfigNotExist,
    #[error("Failed to send config")]
    SendError,
}

/// Represents a reloadable configuration.
#[derive(Debug, Clone)]
pub struct ReloadableConfig {
    pub id: ConfigId,
    pub path: PathBuf,
    pub format: Format,
    pub poll_interval: Duration,
}

impl Reloadify {
    /// Creates a new `Reloadify` instance.
    pub fn new() -> Self {
        Self(Arc::new(RwLock::new(HashMap::new())))
    }

    /// Adds a reloadable configuration to the `Reloadify` instance.
    ///
    /// # Arguments
    ///
    /// * `reloadable_config` - The reloadable configuration to add.
    ///
    /// # Returns
    ///
    /// Returns a result containing the deserialized configuration if successful, or an error if an
    /// error occurred.
    #[allow(unreachable_code, unused_variables)]
    pub fn add<C>(&self, reloadable_config: ReloadableConfig) -> Result<Receiver<C>, ReloadifyError>
    where
        C: DeserializeOwned + Send + Sync + Clone + 'static,
    {
        let initial_cfg =
            self.load::<C>(reloadable_config.path.as_path(), &reloadable_config.format)?;
        let (tx, rx) = channel();
        tx.send(initial_cfg.clone()).map_err(|_| ReloadifyError::SendError)?;

        let c = reloadable_config.clone();
        let s = self.clone();
        let mut watcher = RecommendedWatcher::new(
            move |r: Result<Event, Error>| {
                if_chain!(
                    if let Ok(event) = r;
                    if let EventKind::Modify(ModifyKind::Data(chg)) = event.kind;
                    if chg == DataChange::Content;
                    if let Ok(latest_cfg) = s.load::<C>(c.path.as_path(), &c.format);
                    if let Ok(mut guard) = s.0.write();
                    if let Some(current_cfg) = guard.get_mut(&c.id);
                    then {
                        current_cfg.value = Box::new(latest_cfg.clone());
                        let _ = tx.send(latest_cfg);
                    }
                );
            },
            notify::Config::default().with_poll_interval(reloadable_config.poll_interval),
        )
        .map_err(ReloadifyError::WatchError)?;

        watcher
            .watch(reloadable_config.path.as_path(), notify::RecursiveMode::NonRecursive)
            .map_err(ReloadifyError::WatchError)?;

        let mut guard = self.0.write().map_err(|_| ReloadifyError::GetLockError)?;
        guard
            .entry(reloadable_config.id)
            .or_insert(Config { value: Box::new(initial_cfg), _watcher: watcher });

        Ok(rx)
    }

    /// Retrieves a configuration from the `Reloadify` instance.
    ///
    /// # Arguments
    ///
    /// * `config_id` - The identifier of the configuration to retrieve.
    ///
    /// # Returns
    ///
    /// Returns a result containing the deserialized configuration if it exists, or an error if the
    /// configuration does not exist or an error occurred.
    pub fn get<C>(&self, config_id: ConfigId) -> Result<C, ReloadifyError>
    where
        C: DeserializeOwned + Send + Sync + Clone + 'static,
    {
        match self.0.read() {
            Err(_) => Err(ReloadifyError::GetLockError),
            Ok(guard) => Ok(guard
                .get(&config_id)
                .ok_or(ReloadifyError::ConfigNotExist)?
                .value
                .downcast_ref::<C>()
                .cloned()
                .ok_or(ReloadifyError::DowncastError)?),
        }
    }

    #[allow(unused_variables)]
    fn load<C: DeserializeOwned>(&self, path: &Path, format: &Format) -> Result<C, ReloadifyError> {
        #[cfg(any(
            feature = "json",
            feature = "yaml",
            feature = "toml",
            feature = "xml",
            feature = "ini"
        ))]
        {
            let content = fs::read_to_string(path).map_err(ReloadifyError::LoadConfigError)?;
            match format {
                #[cfg(feature = "json")]
                Format::Json => serde_json::from_str::<C>(&content)
                    .map_err(|err| ReloadifyError::DeserializeError(err.to_string())),
                #[cfg(feature = "yaml")]
                Format::Yaml => serde_yaml::from_str::<C>(&content)
                    .map_err(|err| ReloadifyError::DeserializeError(err.to_string())),
                #[cfg(feature = "toml")]
                Format::Toml => serde_toml::from_str::<C>(&content)
                    .map_err(|err| ReloadifyError::DeserializeError(err.to_string())),
                #[cfg(feature = "xml")]
                Format::Xml => serde_xml::from_str::<C>(&content)
                    .map_err(|err| ReloadifyError::DeserializeError(err.to_string())),
                #[cfg(feature = "ini")]
                Format::Ini => serde_ini::from_str::<C>(&content)
                    .map_err(|err| ReloadifyError::DeserializeError(err.to_string())),
            }
        }

        #[cfg(not(any(
            feature = "json",
            feature = "yaml",
            feature = "toml",
            feature = "xml",
            feature = "ini"
        )))]
        Err(ReloadifyError::DeserializeError("No format feature enabled".to_string()))
    }
}

impl Default for Reloadify {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::HashMap, path::PathBuf, time::Duration};

    /// Build the absolute path to an example config fixture.
    fn fixture(filename: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples").join("config").join(filename)
    }

    // ---------------------------------------------------------------------------
    // Untyped helpers (no format feature needed)
    // ---------------------------------------------------------------------------

    #[test]
    fn new_and_get_nonexistent() {
        let r = Reloadify::new();
        let result = r.get::<String>(ConfigId::new("nonexistent"));
        assert!(matches!(result, Err(ReloadifyError::ConfigNotExist)));
    }

    #[test]
    fn default_constructs() {
        let r = Reloadify::default();
        assert!(r.get::<String>(ConfigId::new("any")).is_err());
    }

    #[test]
    fn config_id_equality() {
        let a = ConfigId::new("foo");
        let b = ConfigId::new("foo");
        let c = ConfigId::new("bar");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn config_id_from_string() {
        let id = ConfigId::new(String::from("owned"));
        assert_eq!(id, ConfigId::new("owned"));
    }

    #[test]
    #[cfg(feature = "json")]
    fn config_id_serde_roundtrip() {
        let id = ConfigId::new("my-config");
        let json = serde_json::to_string(&id).unwrap();
        let back: ConfigId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    // ---------------------------------------------------------------------------
    // JSON
    // ---------------------------------------------------------------------------

    #[cfg(feature = "json")]
    mod json {
        use super::*;
        use serde::Deserialize;

        #[derive(Debug, Clone, Deserialize, PartialEq)]
        struct TsConfig {
            extends: String,
            #[serde(rename = "compilerOptions")]
            compiler_options: CompilerOptions,
            files: Vec<String>,
            include: Vec<String>,
        }

        #[derive(Debug, Clone, Deserialize, PartialEq)]
        struct CompilerOptions {
            #[serde(rename = "outDir")]
            out_dir: String,
            types: Vec<String>,
        }

        #[test]
        fn load_json_file() {
            let r = Reloadify::new();
            let cfg: TsConfig = r
                .load(&fixture("tsconfig.spec.json"), &Format::Json)
                .expect("should load JSON config");
            assert_eq!(cfg.extends, "./tsconfig.json");
            assert_eq!(cfg.compiler_options.out_dir, "./out-tsc/spec");
            assert_eq!(cfg.compiler_options.types, vec!["jasmine"]);
            assert_eq!(cfg.files.len(), 2);
            assert_eq!(cfg.include.len(), 2);
        }

        #[test]
        fn load_missing_file() {
            let r = Reloadify::new();
            let result = r.load::<TsConfig>(
                &PathBuf::from("/nonexistent/reloadify_test.json"),
                &Format::Json,
            );
            assert!(matches!(result, Err(ReloadifyError::LoadConfigError(_))));
        }

        #[test]
        fn load_malformed_json() {
            let tmp = std::env::temp_dir().join("reloadify_test_malformed.json");
            std::fs::write(&tmp, "{ not valid json }").unwrap();
            let r = Reloadify::new();
            let result = r.load::<serde_json::Value>(&tmp, &Format::Json);
            let _ = std::fs::remove_file(&tmp);
            assert!(
                matches!(result, Err(ReloadifyError::DeserializeError(_))),
                "expected DeserializeError, got {result:?}"
            );
        }

        #[test]
        fn add_and_get_roundtrip() {
            let r = Reloadify::new();
            let id = ConfigId::new("tsconfig");
            let rx = r
                .add::<TsConfig>(ReloadableConfig {
                    id: id.clone(),
                    path: fixture("tsconfig.spec.json"),
                    format: Format::Json,
                    poll_interval: Duration::from_secs(10),
                })
                .expect("add should succeed");
            let retrieved = r.get::<TsConfig>(id).expect("get should succeed");
            assert_eq!(retrieved.extends, "./tsconfig.json");
            // channel must carry the initial value
            let from_rx = rx.try_recv().expect("initial value on channel");
            assert_eq!(from_rx.extends, "./tsconfig.json");
        }
    }

    // ---------------------------------------------------------------------------
    // YAML
    // ---------------------------------------------------------------------------

    #[cfg(feature = "yaml")]
    mod yaml {
        use super::*;

        #[test]
        fn load_yaml_file() {
            let r = Reloadify::new();
            let cfg: serde_yaml::Value = r
                .load(&fixture("docker-compose.yaml"), &Format::Yaml)
                .expect("should load YAML config");
            let services = cfg.get("services").expect("should have services key");
            assert!(services.get("minecraft").is_some());
        }

        #[test]
        fn add_and_get_roundtrip() {
            let r = Reloadify::new();
            let id = ConfigId::new("compose");
            let _rx = r
                .add::<serde_yaml::Value>(ReloadableConfig {
                    id: id.clone(),
                    path: fixture("docker-compose.yaml"),
                    format: Format::Yaml,
                    poll_interval: Duration::from_secs(10),
                })
                .expect("add should succeed");
            let retrieved = r.get::<serde_yaml::Value>(id).expect("get should succeed");
            assert!(retrieved.get("services").is_some());
        }

        #[test]
        fn load_missing_file() {
            let r = Reloadify::new();
            let result = r.load::<serde_yaml::Value>(
                &PathBuf::from("/nonexistent/reloadify_test.yaml"),
                &Format::Yaml,
            );
            assert!(matches!(result, Err(ReloadifyError::LoadConfigError(_))));
        }
    }

    // ---------------------------------------------------------------------------
    // TOML
    // ---------------------------------------------------------------------------

    #[cfg(feature = "toml")]
    mod toml_format {
        use super::*;

        #[test]
        fn load_toml_file() {
            let r = Reloadify::new();
            let cfg: toml::Value =
                r.load(&fixture("netlify.toml"), &Format::Toml).expect("should load TOML config");
            let build = cfg.get("build").expect("should have [build] section");
            assert_eq!(build.get("publish").and_then(|v| v.as_str()), Some("public"));
        }

        #[test]
        fn add_and_get_roundtrip() {
            let r = Reloadify::new();
            let id = ConfigId::new("netlify");
            let _rx = r
                .add::<toml::Value>(ReloadableConfig {
                    id: id.clone(),
                    path: fixture("netlify.toml"),
                    format: Format::Toml,
                    poll_interval: Duration::from_secs(10),
                })
                .expect("add should succeed");
            let retrieved = r.get::<toml::Value>(id).expect("get should succeed");
            assert!(retrieved.get("build").is_some());
        }

        #[test]
        fn load_missing_file() {
            let r = Reloadify::new();
            let result = r.load::<toml::Value>(
                &PathBuf::from("/nonexistent/reloadify_test.toml"),
                &Format::Toml,
            );
            assert!(matches!(result, Err(ReloadifyError::LoadConfigError(_))));
        }
    }

    // ---------------------------------------------------------------------------
    // INI
    // ---------------------------------------------------------------------------

    #[cfg(feature = "ini")]
    mod ini {
        use super::*;

        /// `serde_ini` deserializes INI as nested `HashMap`s.
        type IniMap = HashMap<String, HashMap<String, Option<String>>>;

        #[test]
        fn load_ini_file() {
            let r = Reloadify::new();
            let cfg: IniMap =
                r.load(&fixture("pytest.ini"), &Format::Ini).expect("should load INI config");
            let pytest = cfg.get("pytest").expect("should have [pytest] section");
            assert_eq!(pytest.get("addopts").and_then(|v| v.as_deref()), Some("--tb=short -rxs"));
            assert_eq!(
                pytest.get("junit_suite_name").and_then(|v| v.as_deref()),
                Some("docker-py")
            );
        }

        #[test]
        fn add_and_get_roundtrip() {
            let r = Reloadify::new();
            let id = ConfigId::new("pytest");
            let _rx = r
                .add::<IniMap>(ReloadableConfig {
                    id: id.clone(),
                    path: fixture("pytest.ini"),
                    format: Format::Ini,
                    poll_interval: Duration::from_secs(10),
                })
                .expect("add should succeed");
            let retrieved = r.get::<IniMap>(id).expect("get should succeed");
            assert!(retrieved.contains_key("pytest"));
        }

        #[test]
        fn load_missing_file() {
            let r = Reloadify::new();
            let result =
                r.load::<IniMap>(&PathBuf::from("/nonexistent/reloadify_test.ini"), &Format::Ini);
            assert!(matches!(result, Err(ReloadifyError::LoadConfigError(_))));
        }
    }

    // ---------------------------------------------------------------------------
    // XML
    // ---------------------------------------------------------------------------

    #[cfg(feature = "xml")]
    mod xml {
        use super::*;
        use serde::Deserialize;

        #[derive(Debug, Clone, Deserialize, PartialEq)]
        struct TomcatUsers {
            user: Vec<User>,
            role: Vec<Role>,
        }

        #[derive(Debug, Clone, Deserialize, PartialEq)]
        struct User {
            username: String,
            password: String,
            roles: String,
        }

        #[derive(Debug, Clone, Deserialize, PartialEq)]
        struct Role {
            rolename: String,
        }

        #[test]
        fn load_xml_file() {
            let r = Reloadify::new();
            let cfg: TomcatUsers =
                r.load(&fixture("tomcat-users.xml"), &Format::Xml).expect("should load XML config");
            assert_eq!(cfg.user.len(), 2);
            assert_eq!(cfg.user[0].username, "system");
            assert_eq!(cfg.user[0].password, "manager");
            assert_eq!(cfg.user[0].roles, "admin-gui,manager-gui");
            assert_eq!(cfg.role.len(), 1);
            assert_eq!(cfg.role[0].rolename, "manager-script");
        }

        #[test]
        fn add_and_get_roundtrip() {
            let r = Reloadify::new();
            let id = ConfigId::new("tomcat");
            let _rx = r
                .add::<TomcatUsers>(ReloadableConfig {
                    id: id.clone(),
                    path: fixture("tomcat-users.xml"),
                    format: Format::Xml,
                    poll_interval: Duration::from_secs(10),
                })
                .expect("add should succeed");
            let retrieved = r.get::<TomcatUsers>(id).expect("get should succeed");
            assert_eq!(retrieved.user.len(), 2);
            assert_eq!(retrieved.role.len(), 1);
        }

        #[test]
        fn load_missing_file() {
            let r = Reloadify::new();
            let result = r.load::<TomcatUsers>(
                &PathBuf::from("/nonexistent/reloadify_test.xml"),
                &Format::Xml,
            );
            assert!(matches!(result, Err(ReloadifyError::LoadConfigError(_))));
        }
    }

    // ---------------------------------------------------------------------------
    // Multi-config integration
    // ---------------------------------------------------------------------------

    #[test]
    #[cfg(all(feature = "json", feature = "yaml"))]
    fn multiple_configs_in_one_reloadify() {
        let r = Reloadify::new();

        let json_id = ConfigId::new("multi-json");
        let yaml_id = ConfigId::new("multi-yaml");

        r.add::<serde_json::Value>(ReloadableConfig {
            id: json_id.clone(),
            path: fixture("tsconfig.spec.json"),
            format: Format::Json,
            poll_interval: Duration::from_secs(10),
        })
        .expect("add json");

        r.add::<serde_yaml::Value>(ReloadableConfig {
            id: yaml_id.clone(),
            path: fixture("docker-compose.yaml"),
            format: Format::Yaml,
            poll_interval: Duration::from_secs(10),
        })
        .expect("add yaml");

        let json_cfg = r.get::<serde_json::Value>(json_id).expect("get json");
        let yaml_cfg = r.get::<serde_yaml::Value>(yaml_id).expect("get yaml");

        assert!(json_cfg.get("extends").is_some());
        assert!(yaml_cfg.get("services").is_some());
    }

    #[test]
    fn downcast_error_on_wrong_type() {
        let r = Reloadify::new();
        let id = ConfigId::new("typed");

        // We need at least one format feature to call `add`. Use the simplest.
        #[cfg(feature = "json")]
        {
            let _rx = r
                .add::<serde_json::Value>(ReloadableConfig {
                    id: id.clone(),
                    path: fixture("tsconfig.spec.json"),
                    format: Format::Json,
                    poll_interval: Duration::from_secs(10),
                })
                .expect("add should succeed");

            // ask for a *different* type → downcast must fail
            let result = r.get::<HashMap<String, String>>(id);
            assert!(matches!(result, Err(ReloadifyError::DowncastError)));
        }
    }
}
