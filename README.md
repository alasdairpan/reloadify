# Reloadify 🔁

Reloadify is a Rust library designed to facilitate automatic reloading of configuration files in applications. It simplifies the process of detecting changes in configuration files (such as JSON, TOML, XML, etc.) and automatically applying those changes without requiring application restarts.

## Features ✨

- [x] **Automatic Reloading**: Detects changes in configuration files and automatically reloads them.
- [x] **Supports Multiple Formats**: Works with JSON, TOML, XML, and more.
- [x] **Easy Integration**: Designed for seamless integration into Rust applications.
- [x] **Live Changes**: Returns a configuration receiving channel. When the configuration changes, the caller will receive latest configuration.

## Installation 🚀

To use Reloadify in your Rust project, simply add it to your `Cargo.toml`:

```toml
[dependencies]
reloadify = "0.1"
```

## Usage 🛠️

Here's a basic example demonstrating how to use Reloadify to automatically reload a JSON configuration file:

```rust
use reloadify::{ConfigId, Format, ReloadableConfig, Reloadify};
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
        id: ConfigId::new(TS_CONFIG_ID),
        path: PathBuf::from_str("examples/config/tsconfig.spec.json")?,
        format: Format::Json,
        poll_interval: Duration::from_secs(1),
    })?;

    // Optional: Spawn a thread to listen for the latest configuration.
    std::thread::spawn(move || {
        for latest_cfg in rx {
            // Do something with the latest configuration...
            println!("Received latest config: {:?}", latest_cfg);
        }
    });

    let ts_config = reloadify.get::<TsConfig>(ConfigId::new(TS_CONFIG_ID))?;

    // Do something with ts_config...
    println!("{:?}", ts_config);

    Ok(())
}

```

## Documentation 📚

For detailed usage instructions and API reference, see the [documentation](https://docs.rs/reloadify/).

## Contributing 🤝

Contributions are welcome! Please fork the repository and submit a pull request with your changes.

## Continuous Integration 🔄

This project uses GitHub Actions for CI/CD. The pipeline includes:

- Running tests on multiple platforms (Ubuntu, Windows, macOS)
- Testing against different Rust versions (stable, beta, nightly)
- Code formatting checks with `rustfmt`
- Linting with `clippy`
- Security audits with `cargo-audit`
- Documentation generation and deployment

## License 📝

This project is licensed under the MIT License - see the [LICENSE](./LICENSE) file for details.
