use dotenv::dotenv;
use lazy_static::lazy_static;
use serde::Deserialize;

fn default_namespace() -> String {
    "default".to_string()
}

fn default_bucket() -> String {
    "leaks-bucket".to_string()
}

fn default_scope() -> String {
    "_default".to_string()
}

fn default_collection() -> String {
    "leaks".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub couch_uri: String,
    pub couch_username: String,
    pub couch_password: String,
    #[serde(default = "default_namespace")]
    pub couch_namespace: String,
    #[serde(default = "default_bucket")]
    pub couch_bucket: String,
    #[serde(default = "default_scope")]
    pub couch_scope: String,
    #[serde(default = "default_collection")]
    pub couch_collection: String,
    pub tld_path: String,
}

fn init_config() -> Config {
    dotenv().ok();

    match envy::from_env::<Config>() {
        Ok(config) => config,
        Err(err) => panic!("Couldn't process env variables: {:#?}", err),
    }
}

lazy_static! {
    pub static ref CONFIG: Config = init_config();
}
