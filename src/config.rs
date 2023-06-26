use base64::{engine::general_purpose as b64, Engine as _};
use rocket::http::Status;
use rocket::request::{FromRequest, Outcome, Request};
use rocket::response::status::Forbidden;
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

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Auth {
    type Error = Forbidden<&'static str>;

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let config = request.guard::<Config>().await.unwrap();
        let mut req_auth = Auth {
            basic: None,
            bearer: None,
        };
        if let Some(conf_auth) = config.auth {
            let authz = request.headers().get_one("authorization");
            req_auth.basic = authz
                .map(|value| {
                    if value.len() < 7 || &value[..6] != "Basic " {
                        return None;
                    }
                    let decoded = match b64::STANDARD_NO_PAD.decode(&value[6..]) {
                        Ok(bytes) => String::from_utf8(bytes).unwrap(),
                        Err(_) => return None,
                    };

                    decoded.split_once(":").map(|(user, pass)| BasicAuth {
                        user: user.to_string(),
                        pass: pass.to_string(),
                    })
                })
                .flatten();

            req_auth.bearer = authz
                .map(|value| {
                    if value.len() < 7 || &value[..7] != "Bearer " {
                        return None;
                    }

                    Some(BearerAuth {
                        token: value[7..].to_string(),
                    })
                })
                .flatten();

            let basic_ok = conf_auth.basic != None && conf_auth.basic == req_auth.basic;
            let bearer_ok = conf_auth.bearer != None && conf_auth.bearer == req_auth.bearer;
            if !basic_ok && !bearer_ok {
                return Outcome::Failure((
                    rocket::http::Status { code: 403 },
                    Forbidden(Some("Not authorized.")),
                ));
            }
        }
        return Outcome::Success(req_auth);
    }
}
