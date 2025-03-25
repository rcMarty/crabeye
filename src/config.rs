use anyhow::Context;
use secrecy::SecretString;
use serde::Deserialize;
use std::env;

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(skip_deserializing)]
    pub github_token: SecretString,
    pub db_url: String,
    pub repo_owner: String,
    pub repo_name: String,
    pub log_level: String,
    // pub webhook_secret: Option<SecretString>,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        dotenvy::dotenv().ok();

        Ok(Config {
            github_token: SecretString::new(Box::from(
                env::var("GITHUB_TOKEN").context("Missing GITHUB_TOKEN")?,
            )),
            db_url: env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:data.db".into()),
            repo_owner: env::var("REPO_OWNER").context("Missing REPO_OWNER")?,
            repo_name: env::var("REPO_NAME").context("Missing REPO_NAME")?,
            log_level: env::var("LOG_LEVEL").unwrap_or_else(|_| "info".into()),
            // webhook_secret: env::var("WEBHOOK_SECRET")
            //     .map(SecretString::new).ok(),
        })
    }
}
