//! Google Translate: keyless public endpoint and official Cloud API v2.

use newtrans_core::lang::normalize;
use reqwest::{Client, RequestBuilder};
use serde_json::Value;

use crate::{ProviderError, Request, Translation};

pub(crate) const FREE_URL: &str = "https://translate.googleapis.com/translate_a/single";
pub(crate) const CLOUD_URL: &str = "https://translation.googleapis.com/language/translate/v2";

fn code(code: &str) -> String {
    // Google accepts catalog codes as-is ("zh-CN", "zh-TW").
    code.to_owned()
}

pub(crate) fn free_request(client: &Client, request: &Request<'_>) -> RequestBuilder {
    let source = request
        .source
        .map(code)
        .unwrap_or_else(|| "auto".to_owned());
    client
        .post(FREE_URL)
        .query(&[
            ("client", "gtx"),
            ("sl", source.as_str()),
            ("tl", &code(request.target)),
            ("dt", "t"),
        ])
        .form(&[("q", request.text)])
}

/// The keyless endpoint returns nested arrays:
/// `[[["번역", "source", ...], ...], null, "en", ...]`.
pub(crate) fn parse_free(body: &str) -> Result<Translation, ProviderError> {
    let value: Value = serde_json::from_str(body).map_err(|_| ProviderError::BadResponse)?;
    let segments = value
        .get(0)
        .and_then(Value::as_array)
        .ok_or(ProviderError::BadResponse)?;
    let text: String = segments
        .iter()
        .filter_map(|segment| segment.get(0).and_then(Value::as_str))
        .collect();
    let detected = value.get(2).and_then(Value::as_str).map(normalize);
    Ok(Translation { text, detected })
}

pub(crate) fn cloud_request(
    client: &Client,
    api_key: &str,
    request: &Request<'_>,
) -> Result<RequestBuilder, ProviderError> {
    if api_key.trim().is_empty() {
        return Err(ProviderError::MissingKey);
    }
    let mut body = serde_json::json!({
        "q": [request.text],
        "target": code(request.target),
        "format": "text",
    });
    if let Some(source) = request.source {
        body["source"] = Value::String(code(source));
    }
    // The key goes in a header, not the URL, so it never lands in proxy logs.
    Ok(client
        .post(CLOUD_URL)
        .header("X-Goog-Api-Key", api_key.trim())
        .json(&body))
}

pub(crate) fn parse_cloud(body: &str) -> Result<Translation, ProviderError> {
    let value: Value = serde_json::from_str(body).map_err(|_| ProviderError::BadResponse)?;
    let first = value
        .pointer("/data/translations/0")
        .ok_or(ProviderError::BadResponse)?;
    let text = first
        .get("translatedText")
        .and_then(Value::as_str)
        .ok_or(ProviderError::BadResponse)?;
    let detected = first
        .get("detectedSourceLanguage")
        .and_then(Value::as_str)
        .map(normalize);
    Ok(Translation {
        text: text.to_owned(),
        detected,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_client;

    #[test]
    fn free_request_uses_auto_and_form_body() {
        let client = test_client();
        let request = Request {
            text: "Hello & bye",
            source: None,
            target: "ko",
        };
        let built = free_request(&client, &request).build().unwrap();
        let url = built.url().as_str();
        assert!(url.starts_with(FREE_URL));
        assert!(url.contains("sl=auto") && url.contains("tl=ko") && url.contains("client=gtx"));
        let body = std::str::from_utf8(built.body().unwrap().as_bytes().unwrap()).unwrap();
        assert_eq!(body, "q=Hello+%26+bye");
    }

    #[test]
    fn parses_free_response_segments() {
        let body = r#"[[["안녕하세요. ","Hello. ",null,null,10],["세계","world",null,null,10]],null,"en",null,null,null,1]"#;
        let parsed = parse_free(body).unwrap();
        assert_eq!(parsed.text, "안녕하세요. 세계");
        assert_eq!(parsed.detected.as_deref(), Some("en"));
        assert_eq!(parse_free("<html>"), Err(ProviderError::BadResponse));
    }

    #[test]
    fn cloud_request_requires_key_and_hides_it_from_url() {
        let client = test_client();
        let request = Request {
            text: "Hi",
            source: Some("en"),
            target: "ko",
        };
        assert!(matches!(
            cloud_request(&client, " ", &request),
            Err(ProviderError::MissingKey)
        ));
        let built = cloud_request(&client, "KEY123", &request)
            .unwrap()
            .build()
            .unwrap();
        assert!(!built.url().as_str().contains("KEY123"));
        assert_eq!(built.headers()["X-Goog-Api-Key"], "KEY123");
        let body: Value =
            serde_json::from_slice(built.body().unwrap().as_bytes().unwrap()).unwrap();
        assert_eq!(body["source"], "en");
        assert_eq!(body["q"][0], "Hi");
    }

    #[test]
    fn parses_cloud_response() {
        let body = r#"{"data":{"translations":[{"translatedText":"안녕","detectedSourceLanguage":"en"}]}}"#;
        let parsed = parse_cloud(body).unwrap();
        assert_eq!(parsed.text, "안녕");
        assert_eq!(parsed.detected.as_deref(), Some("en"));
    }
}
