use rocket::http::Status;
use rocket::request::{FromRequest, Outcome, Request};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::net::Ipv4Addr;
use toml;

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
#[serde(deny_unknown_fields)]
pub enum Deployment {
    Command {
        name: String,
        command: String,
        #[serde(default)]
        args: Vec<String>,
    },
    Script {
        name: String,
        script: String,
    },
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct BasicAuth {
    pub user: String,
    pub pass: String,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct BearerAuth {
    pub token: String,
}

#[derive(Debug, Deserialize)]
pub struct Auth {
    #[serde(default)]
    pub basic: Option<BasicAuth>,
    #[serde(default)]
    pub bearer: Option<BearerAuth>,
}

fn default_host() -> Ipv4Addr {
    Ipv4Addr::new(0, 0, 0, 0)
}
fn default_port() -> u16 {
    6391
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawConfig {
    #[serde(default = "default_host")]
    pub host: Ipv4Addr,
    #[serde(default = "default_port")]
    pub port: u16,

    #[serde(default)]
    pub auth: Option<Auth>,

    #[serde(default)]
    pub deployment: Vec<Deployment>,
}

#[derive(Debug)]
pub struct Config {
    pub host: Ipv4Addr,
    pub port: u16,

    pub auth: Option<Auth>,

    pub deployment: HashMap<String, Deployment>,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            host: default_host(),
            port: default_port(),
            auth: None,
            deployment: HashMap::new(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Responder)]
#[response(status = 500, content_type = "json")]
pub enum ConfigError {
    ReadFileError(String),
    ConfigDeserializeError(String),
}

pub type ConfigFile = String;

pub fn load_file(path: ConfigFile) -> Result<Config, ConfigError> {
    let contents = fs::read_to_string(path.clone()).map_err(|err| {
        eprintln!("Error from fs::read_to_string: {:?}", err);
        ConfigError::ReadFileError(format!("Could not read file {}", path))
    })?;
    let raw_config: RawConfig = toml::from_str(&contents).map_err(|err| {
        eprintln!("Error from toml::from_str: {}", err.to_string());
        ConfigError::ConfigDeserializeError(format!(
            "Could not parse file {}: {}",
            path,
            err.to_string()
        ))
    })?;

    let config = Config {
        host: raw_config.host,
        port: raw_config.port,
        auth: raw_config.auth,
        deployment: raw_config
            .deployment
            .into_iter()
            .map(|each| match each {
                Deployment::Script { name, script } => {
                    (name.clone(), Deployment::Script { name, script })
                }
                Deployment::Command {
                    name,
                    command,
                    args,
                } => (
                    name.clone(),
                    Deployment::Command {
                        name,
                        command,
                        args,
                    },
                ),
            })
            .collect(),
    };

    Ok(config)
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Config {
    type Error = ConfigError;

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let config_file: ConfigFile = request.rocket().state::<ConfigFile>().unwrap().clone();

        match load_file(config_file) {
            Ok(conf) => Outcome::Success(conf),
            Err(err) => Outcome::Failure((Status::InternalServerError, err)),
        }
    }
}
