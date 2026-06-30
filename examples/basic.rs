use reloadify::{Format, ReloadableConfig, Reloadify};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, str::FromStr, time::Duration};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TsConfig {
    pub extends: String,
    #[serde(rename = "compilerOptions")]
    pub compiler_options: CompilerOptions,
    pub files: Vec<String>,
    pub include: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompilerOptions {
    #[serde(rename = "outDir")]
    pub out_dir: String,
    pub types: Vec<String>,
}

const TS_CONFIG_ID: &str = "tsconfig";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let reloadify = Reloadify::new();

    let rx = reloadify.add::<TsConfig>(ReloadableConfig {
        id: TS_CONFIG_ID.into(),
        path: PathBuf::from_str("examples/config/tsconfig.spec.json")?,
        format: Format::Json,
        poll_interval: Duration::from_secs(1),
        debounce_delay: None,
    })?;

    // Optional: Spawn a thread to listen for the latest configuration.
    std::thread::spawn(move || {
        for latest_cfg in rx {
            // Do something with the latest configuration...
            println!("Received latest config: {:?}", latest_cfg);
        }
    });

    let ts_config = reloadify.get::<TsConfig>(TS_CONFIG_ID)?;

    // Do something with ts_config...
    println!("{:?}", ts_config);

    Ok(())
}
