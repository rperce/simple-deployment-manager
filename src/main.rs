#[macro_use]
extern crate rocket;

use chrono::{DateTime, Utc};
use rocket::http;
use rocket::serde::json::Json;
use rocket::State;
use serde::Serialize;
use std::collections::HashMap;
use std::process::Command;
use std::sync::{Arc, RwLock};

mod config;
use config::{Auth, Config, ConfigFile, Deployment};

#[derive(Clone, Debug, Default, Serialize)]
pub enum DeploymentState {
    #[default]
    NeverDeployed,
    InProgress,
    Failed,
    Completed,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct DeploymentStatus {
    name: String,
    state: DeploymentState,
    last_deployed: Option<DateTime<Utc>>,
    stdout: Option<String>,
    stderr: Option<String>,
    exit_code: Option<i32>,
}

pub type DeploymentStatusMap = Arc<RwLock<HashMap<String, DeploymentStatus>>>;

#[post("/deploy/<name>")]
fn deploy(
    name: &str,
    config: Config,
    status_map: &State<DeploymentStatusMap>,
    _auth: Auth,
) -> Option<Json<DeploymentStatus>> {
    config.deployment.get(name).map(|deployment| {
        let status = DeploymentStatus {
            name: name.to_string(),
            state: DeploymentState::InProgress,
            ..Default::default()
        };
        status_map
            .write()
            .unwrap()
            .insert(name.to_string(), status.clone());

        let deployment = deployment.clone();
        let name = name.to_string();
        let status_map = status_map.inner().clone();
        rocket::tokio::spawn(async move {
            let now = Utc::now();
            let output = match deployment {
                Deployment::Script { script, .. } => Command::new("/usr/bin/env")
                    .arg("bash")
                    .arg(script)
                    .output(),
                Deployment::Command { command, args, .. } => {
                    Command::new(command.clone()).args(args.clone()).output()
                }
            };

            match output {
                Err(_) => {
                    status_map.write().unwrap().insert(
                        name.to_string(),
                        DeploymentStatus {
                            name: name.to_string(),
                            state: DeploymentState::Failed,
                            last_deployed: Some(now),
                            ..Default::default()
                        },
                    );
                }
                Ok(output) => {
                    status_map.write().unwrap().insert(
                        name.to_string(),
                        DeploymentStatus {
                            name: name.to_string(),
                            state: DeploymentState::Completed,
                            last_deployed: Some(now),
                            stdout: Some(
                                String::from_utf8(output.stdout)
                                    .unwrap_or("Error: invalid UTF8!".to_string()),
                            ),
                            stderr: Some(
                                String::from_utf8(output.stderr)
                                    .unwrap_or("Error: invalid UTF8!".to_string()),
                            ),
                            exit_code: output.status.code(),
                        },
                    );
                }
            }
        });

        Json(status)
    })
}

#[get("/status/<name>")]
fn status(
    name: &str,
    status_map: &State<DeploymentStatusMap>,
    _auth: Auth,
) -> Option<(http::Status, Json<DeploymentStatus>)> {
    status_map.read().unwrap().get(name).map(|status| {
        let status_code = match status.state {
            DeploymentState::Failed => http::Status::InternalServerError,
            _ => http::Status::Ok,
        };
        (status_code, Json(status.clone()))
    })
}

#[launch]
fn rocket() -> _ {
    let path: ConfigFile = "./example.toml".to_string();
    let conf = config::load_file(path.clone()).unwrap();
    println!("conf {:?}", conf);
    let rocket_config = rocket::Config {
        port: conf.port,
        address: conf.host.into(),
        ..rocket::Config::default()
    };

    let deployment_status: DeploymentStatusMap = Arc::new(RwLock::new(HashMap::new()));
    rocket::custom(&rocket_config)
        .manage(path)
        .manage(deployment_status)
        .mount("/", routes![deploy, status])
}
