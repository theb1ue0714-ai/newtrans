//! Live checks against real services. Ignored by default; run with
//! `cargo test -p newtrans-providers --test live -- --ignored`.
//! Keys are read from the environment only (NEWTRANS_DEEPL_API_KEY,
//! NEWTRANS_PAPAGO_CLIENT_ID/SECRET, NEWTRANS_GOOGLE_API_KEY).

use std::time::Duration;

use newtrans_providers::{Provider, Request, http_client};

async fn check(provider: Provider) {
    let client = http_client(Duration::from_secs(15)).unwrap();
    let result = provider
        .translate(
            &client,
            &Request {
                text: "Good morning",
                source: None,
                target: "ko",
            },
        )
        .await
        .unwrap();
    assert!(!result.text.is_empty());
    assert_eq!(result.detected.as_deref(), Some("en"));
}

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

#[tokio::test]
#[ignore = "network"]
async fn google_free() {
    check(Provider::GoogleFree).await;
}

#[tokio::test]
#[ignore = "network + key"]
async fn deepl() {
    let Some(api_key) = env("NEWTRANS_DEEPL_API_KEY") else {
        return;
    };
    check(Provider::Deepl { api_key }).await;
}

#[tokio::test]
#[ignore = "network + key"]
async fn papago() {
    let (Some(client_id), Some(client_secret)) = (
        env("NEWTRANS_PAPAGO_CLIENT_ID"),
        env("NEWTRANS_PAPAGO_CLIENT_SECRET"),
    ) else {
        return;
    };
    check(Provider::Papago {
        client_id,
        client_secret,
    })
    .await;
}

#[tokio::test]
#[ignore = "network + key"]
async fn google_cloud() {
    let Some(api_key) = env("NEWTRANS_GOOGLE_API_KEY") else {
        return;
    };
    check(Provider::GoogleCloud { api_key }).await;
}
