use reqwest::RequestBuilder;
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::models::{token::Token, updater::Updater};

pub struct RequestClient<U: Updater> {
    http: reqwest::Client,
    token: Mutex<Option<Token>>,
    pub updater: U,
}

impl<U: Updater> RequestClient<U> {
    pub fn new(updater: U) -> Self {
        Self {
            http: reqwest::Client::new(),
            updater,
            token: tokio::sync::Mutex::new(None),
        }
    }

    pub async fn authorize(&self) -> Result<String, anyhow::Error> {
        let new_token = self.updater.authorize().await?;

        let mut current = self.token.lock().await;
        *current = Some(new_token.clone());

        Ok(new_token.access_token)
    }

    async fn token(&self) -> Result<String, anyhow::Error> {
        let token = {
            let current = self.token.lock().await;
            current.as_ref().map(|tok| tok.access_token.clone())
        };

        let result = match token {
            None => self.authorize().await?,
            Some(token) => token,
        };

        Ok(result)
    }

    pub async fn request<F>(&self, build: F) -> Result<reqwest::Response, anyhow::Error>
    where
        F: Fn(&reqwest::Client, &str) -> RequestBuilder,
    {
        const MAX_RETRIES: u64 = 4;

        for attempt in 0..MAX_RETRIES {
            info!("attempt at making request {}", attempt);

            let token = self.token().await?;
            let request = build(&self.http, &token);

            let response = request
                .header("Authorization", format!("Bearer {}", token))
                .send()
                .await;

            match response {
                Ok(resp) => match resp.error_for_status() {
                    Ok(res) => return Ok(res),
                    Err(err) => {
                        self.authorize().await.ok();
                        warn!("request returned error {:?} {:?}", err.status(), err.url());
                    }
                },
                Err(error) => {
                    warn!("making request failed {:?}", error.url());
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(5 * attempt)).await;
        }

        Err(anyhow::anyhow!(
            "even after 5 retries the request still has failed"
        ))
    }
}
