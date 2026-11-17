//! Translation providers and the three-step chain runner.
//!
//! Every provider is split into a pure request builder and a pure response
//! parser so both are unit tested without network access. Only
//! [`Provider::translate`] performs I/O.

mod chain;
mod deepl;
mod error;
mod google;
mod papago;

use std::time::Duration;

use reqwest::{RequestBuilder, Response};
use serde::Serialize;

pub use chain::{ChainRequest, ChainResult, run_chain};
pub use error::ProviderError;
pub use reqwest::Client;

/// One translation call. `source = None` asks for auto detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request<'a> {
    pub text: &'a str,
    pub source: Option<&'a str>,
    pub target: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Translation {
    pub text: String,
    /// Source language reported by the provider, normalized to catalog form.
    pub detected: Option<String>,
}

/// A configured provider. Keys are moved in from the secrets store for the
/// duration of a request and never logged.
#[derive(Clone)]
pub enum Provider {
    /// Keyless public Google endpoint used by translate.google.com tools.
    GoogleFree,
    /// Official Cloud Translation v2 with the user's API key.
    GoogleCloud { api_key: String },
    /// NAVER Cloud Platform Papago Translation.
    Papago {
        client_id: String,
        client_secret: String,
    },
    /// DeepL API (Free keys end in ":fx").
    Deepl { api_key: String },
}

impl std::fmt::Debug for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Provider::GoogleFree => "GoogleFree",
            Provider::GoogleCloud { .. } => "GoogleCloud",
            Provider::Papago { .. } => "Papago",
            Provider::Deepl { .. } => "Deepl",
        })
    }
}

/// Upper bound accepted by every provider for a single request.
pub const MAX_CHARS: usize = 5000;

pub fn http_client(timeout: Duration) -> reqwest::Result<Client> {
    Client::builder()
        .timeout(timeout)
        .connect_timeout(Duration::from_secs(5))
        .user_agent(concat!("NewTrans/", env!("CARGO_PKG_VERSION")))
        .build()
}

impl Provider {
    pub fn supports(&self, code: &str) -> bool {
        match self {
            Provider::Papago { .. } => papago::code(code).is_some(),
            _ => true,
        }
    }

    pub async fn translate(
        &self,
        client: &Client,
        request: &Request<'_>,
    ) -> Result<Translation, ProviderError> {
        let text: String = request.text.chars().take(MAX_CHARS).collect();
        if text.trim().is_empty() {
            return Ok(Translation {
                text: String::new(),
                detected: None,
            });
        }
        let request = Request {
            text: &text,
            ..request.clone()
        };
        match self {
            Provider::GoogleFree => {
                let body = send(google::free_request(client, &request)).await?;
                google::parse_free(&body)
            }
            Provider::GoogleCloud { api_key } => {
                let body = send(google::cloud_request(client, api_key, &request)?).await?;
                google::parse_cloud(&body)
            }
            Provider::Papago {
                client_id,
                client_secret,
            } => {
                let body =
                    send(papago::request(client, client_id, client_secret, &request)?).await?;
                papago::parse(&body)
            }
            Provider::Deepl { api_key } => {
                let body = send(deepl::request(client, api_key, &request)?).await?;
                deepl::parse(&body)
            }
        }
    }
}

async fn send(builder: RequestBuilder) -> Result<String, ProviderError> {
    let response: Response = builder.send().await.map_err(ProviderError::from_reqwest)?;
    let status = response.status();
    let body = response.text().await.map_err(ProviderError::from_reqwest)?;
    if status.is_success() {
        Ok(body)
    } else {
        Err(ProviderError::from_status(status.as_u16(), &body))
    }
}

#[cfg(test)]
pub(crate) fn test_client() -> Client {
    http_client(Duration::from_secs(5)).unwrap()
}
