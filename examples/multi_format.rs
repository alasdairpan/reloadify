use reloadify::{Format, ReloadableConfig, Reloadify};
use std::{path::PathBuf, str::FromStr, time::Duration};

const COMPOSE_CONFIG_ID: &str = "docker-compose";
const PYTEST_CONFIG_ID: &str = "pytest";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let reloadify = Reloadify::new();

    reloadify.add::<compose::ComposeConfig>(ReloadableConfig {
        id: COMPOSE_CONFIG_ID.into(),
        path: PathBuf::from_str("examples/config/docker-compose.yaml")?,
        format: Format::Yaml,
        debounce_delay: None,
        poll_interval: Duration::from_secs(1),
    })?;
    reloadify.add::<pytest::PyTestConfig>(ReloadableConfig {
        id: PYTEST_CONFIG_ID.into(),
        path: PathBuf::from_str("examples/config/pytest.ini")?,
        format: Format::Ini,
        debounce_delay: None,
        poll_interval: Duration::from_millis(100),
    })?;

    let compose_config = reloadify.get::<compose::ComposeConfig>(COMPOSE_CONFIG_ID)?;
    let pytest_config = reloadify.get::<pytest::PyTestConfig>(PYTEST_CONFIG_ID)?;

    // Do something with compose_config and pytest_config...
    println!("compose_config={compose_config:?}\npytest_config={pytest_config:?}");

    Ok(())
}

mod pytest {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct PyTestConfig {
        pub pytest: PyTest,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct PyTest {
        addopts: String,
        junit_suite_name: String,
        junit_family: String,
    }
}

mod compose {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Clone, Debug)]
    pub struct ComposeConfig {
        #[serde(rename = "services")]
        services: Services,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    pub struct Services {
        #[serde(rename = "minecraft")]
        minecraft: Minecraft,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    pub struct Minecraft {
        #[serde(rename = "image")]
        image: String,
        #[serde(rename = "ports")]
        ports: Vec<String>,
        #[serde(rename = "environment")]
        environment: Environment,
        #[serde(rename = "deploy")]
        deploy: Deploy,
        #[serde(rename = "volumes")]
        volumes: Vec<String>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    pub struct Deploy {
        #[serde(rename = "resources")]
        resources: Resources,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    pub struct Resources {
        #[serde(rename = "limits")]
        limits: Limits,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    pub struct Limits {
        #[serde(rename = "memory")]
        memory: String,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    pub struct Environment {
        #[serde(rename = "EULA")]
        eula: String,
    }
}
