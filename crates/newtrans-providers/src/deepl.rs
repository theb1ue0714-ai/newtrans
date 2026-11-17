//! DeepL API v2. Free keys end in ":fx" and use api-free.deepl.com.

use newtrans_core::lang::normalize;
use reqwest::{Client, RequestBuilder};
use serde_json::Value;

use crate::{ProviderError, Request, Translation};

pub(crate) fn base_url(api_key: &str) -> &'static str {
    if api_key.trim().ends_with(":fx") {
        "https://api-free.deepl.com"
    } else {
        "https://api.deepl.com"
    }
}

pub(crate) fn source_code(code: &str) -> String {
    match normalize(code).as_str() {
        "zh-CN" | "zh-TW" => "ZH".to_owned(),
        other => other.to_ascii_uppercase(),
    }
}

pub(crate) fn target_code(code: &str) -> String {
    match normalize(code).as_str() {
        "en" => "EN-US".to_owned(),
        "pt" => "PT-PT".to_owned(),
        "zh-CN" => "ZH-HANS".to_owned(),
        "zh-TW" => "ZH-HANT".to_owned(),
        other => other.to_ascii_uppercase(),
    }
}

pub(crate) fn request(
    client: &Client,
    api_key: &str,
    request: &Request<'_>,
) -> Result<RequestBuilder, ProviderError> {
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err(ProviderError::MissingKey);
    }
    let mut body = serde_json::json!({
        "text": [request.text],
        "target_lang": target_code(request.target),
    });
    if let Some(source) = request.source {
        body["source_lang"] = Value::String(source_code(source));
    }
    Ok(client
        .post(format!("{}/v2/translate", base_url(api_key)))
        .header("Authorization", format!("DeepL-Auth-Key {api_key}"))
        .json(&body))
}

pub(crate) fn parse(body: &str) -> Result<Translation, ProviderError> {
    let value: Value = serde_json::from_str(body).map_err(|_| ProviderError::BadResponse)?;
    let first = value
        .pointer("/translations/0")
        .ok_or(ProviderError::BadResponse)?;
    let text = first
        .get("text")
        .and_then(Value::as_str)
        .ok_or(ProviderError::BadResponse)?;
    let detected = first
        .get("detected_source_language")
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
    fn picks_endpoint_by_key_type() {
        assert_eq!(base_url("abc:fx"), "https://api-free.deepl.com");
        assert_eq!(base_url("abc"), "https://api.deepl.com");
    }

    #[test]
    fn maps_language_codes() {
        assert_eq!(target_code("en"), "EN-US");
        assert_eq!(target_code("ko"), "KO");
        assert_eq!(target_code("zh-TW"), "ZH-HANT");
        assert_eq!(source_code("zh-CN"), "ZH");
        assert_eq!(source_code("de"), "DE");
    }

    #[test]
    fn request_sets_auth_header_and_body() {
        let client = test_client();
        let req = Request {
            text: "Hallo",
            source: None,
            target: "ko",
        };
        assert!(matches!(
            request(&client, "", &req),
            Err(ProviderError::MissingKey)
        ));
        let built = request(&client, "k:fx", &req).unwrap().build().unwrap();
        assert_eq!(
            built.url().as_str(),
            "https://api-free.deepl.com/v2/translate"
        );
        assert_eq!(built.headers()["Authorization"], "DeepL-Auth-Key k:fx");
        let body: Value =
            serde_json::from_slice(built.body().unwrap().as_bytes().unwrap()).unwrap();
        assert_eq!(body["target_lang"], "KO");
        assert!(body.get("source_lang").is_none());
    }

    #[test]
    fn parses_response() {
        let parsed =
            parse(r#"{"translations":[{"detected_source_language":"DE","text":"안녕"}]}"#).unwrap();
        assert_eq!(parsed.text, "안녕");
        assert_eq!(parsed.detected.as_deref(), Some("de"));
    }
}
