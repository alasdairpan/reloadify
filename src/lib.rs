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
    #[allow(clippy::new_without_default)]
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
        tx.send(initial_cfg.clone())
            .map_err(|_| ReloadifyError::SendError)?;

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
            .watch(
                reloadable_config.path.as_path(),
                notify::RecursiveMode::NonRecursive,
            )
            .map_err(ReloadifyError::WatchError)?;

        let mut guard = self.0.write().map_err(|_| ReloadifyError::GetLockError)?;
        guard.entry(reloadable_config.id).or_insert(Config {
            value: Box::new(initial_cfg),
            _watcher: watcher,
        });

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
                .ok_or_else(|| ReloadifyError::ConfigNotExist)?
                .value
                .downcast_ref::<C>()
                .cloned()
                .ok_or_else(|| ReloadifyError::DowncastError)?),
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
        Err(ReloadifyError::DeserializeError(
            "No format feature enabled".to_string(),
        ))
    }
}
