use crate::config::{Auth, BasicAuth, BearerAuth, Config};
use base64::{engine::general_purpose as b64, Engine as _};
use rocket::request::{FromRequest, Outcome, Request};
use rocket::response::status::Forbidden;

async fn validate_auth(request: &Request<'_>, config: &Config) -> Result<Auth, ()> {
    let mut req_auth = Auth {
        basic: None,
        bearer: None,
    };
    if let Some(conf_auth) = &config.auth {
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
            return Err(());
        }
    }
    Ok(req_auth)
}
#[rocket::async_trait]
impl<'r> FromRequest<'r> for Auth {
    type Error = Forbidden<&'static str>;

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let config = request.guard::<Config>().await.unwrap();
        match validate_auth(request, &config).await {
            Ok(req_auth) => Outcome::Success(req_auth),
            Err(()) => Outcome::Failure((
                rocket::http::Status { code: 403 },
                Forbidden(Some("Not authorized.")),
            )),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use rocket::http::Header;
    use rocket::local::asynchronous::{Client, LocalRequest};

    fn build_req_with_bearer_auth<'a>(client: &'a Client, bearer: BearerAuth) -> LocalRequest<'a> {
        client.get("/").header(Header::new(
            "Authorization",
            format!("Bearer {}", bearer.token),
        ))
    }
    fn build_req_with_basic_auth<'a>(client: &'a Client, basic: BasicAuth) -> LocalRequest<'a> {
        client.get("/").header(Header::new(
            "Authorization",
            format!("Basic {}", b64::STANDARD_NO_PAD.encode(format!("{}:{}", basic.user, basic.pass)))
        ))
    }

    #[rocket::async_test]
    async fn it_accepts_anything_with_no_conf_auth() {
        let client = Client::tracked(rocket::build()).await.unwrap();
        let req = client.get("/");
        let valid = validate_auth(
            &req,
            &Config {
                auth: None,
                ..Default::default()
            },
        )
        .await;
        assert!(valid.is_ok());
    }

    #[rocket::async_test]
    async fn it_accepts_correct_basic_auth() {
        let client = Client::tracked(rocket::build()).await.unwrap();
        let req = build_req_with_basic_auth(&client, BasicAuth { user: "foo".to_string(), pass: "bar".to_string() });
        let valid = validate_auth(
            &req,
            &Config {
                auth: Some(Auth {
                    basic: Some(BasicAuth { user: "foo".to_string(), pass: "bar".to_string() }),
                    bearer: None,
                }),
                ..Default::default()
            },
        )
        .await;
        assert!(valid.is_ok());
    }

    #[rocket::async_test]
    async fn it_rejects_incorrect_basic_auth() {
        let client = Client::tracked(rocket::build()).await.unwrap();
        let req = build_req_with_basic_auth(&client, BasicAuth { user: "foo".to_string(), pass: "bar".to_string() });
        let valid = validate_auth(
            &req,
            &Config {
                auth: Some(Auth {
                    basic: Some(BasicAuth { user: "foo".to_string(), pass: "baz".to_string() }),
                    bearer: None,
                }),
                ..Default::default()
            },
        )
        .await;
        assert!(valid.is_err());
    }

    #[rocket::async_test]
    async fn it_accepts_correct_bearer_auth() {
        let client = Client::tracked(rocket::build()).await.unwrap();
        let req = build_req_with_bearer_auth(&client, BearerAuth { token: "vc6LKWpXprmN0PEUAsvR4qnslzLfKU8xcB2p7Js0QV4=".to_string() });
        let valid = validate_auth(
            &req,
            &Config {
                auth: Some(Auth {
                    basic: None,
                    bearer: Some(BearerAuth { token: "vc6LKWpXprmN0PEUAsvR4qnslzLfKU8xcB2p7Js0QV4=".to_string() }),
                }),
                ..Default::default()
            },
        )
        .await;
        assert!(valid.is_ok());
    }

    #[rocket::async_test]
    async fn it_rejects_incorrect_bearer_auth() {
        let client = Client::tracked(rocket::build()).await.unwrap();
        let req = build_req_with_bearer_auth(&client, BearerAuth { token: "asdf".to_string() });
        let valid = validate_auth(
            &req,
            &Config {
                auth: Some(Auth {
                    basic: None,
                    bearer: Some(BearerAuth { token: "qwerty".to_string() }),
                }),
                ..Default::default()
            },
        )
        .await;
        assert!(valid.is_err());

    }
}
