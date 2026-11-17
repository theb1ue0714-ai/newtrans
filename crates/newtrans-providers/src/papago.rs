//! NAVER Cloud Platform Papago Translation (NMT).

use newtrans_core::lang::normalize;
use reqwest::{Client, RequestBuilder};
use serde_json::Value;

use crate::{ProviderError, Request, Translation};

pub(crate) const URL: &str = "https://papago.apigw.ntruss.com/nmt/v1/translation";

/// Papago supports a fixed list of languages.
pub(crate) fn code(code: &str) -> Option<&'static str> {
    Some(match normalize(code).as_str() {
        "ko" => "ko",
        "en" => "en",
        "ja" => "ja",
        "zh-CN" => "zh-CN",
        "zh-TW" => "zh-TW",
        "vi" => "vi",
        "id" => "id",
        "th" => "th",
        "de" => "de",
        "ru" => "ru",
        "es" => "es",
        "it" => "it",
        "fr" => "fr",
        _ => return None,
    })
}

pub(crate) fn request(
    client: &Client,
    client_id: &str,
    client_secret: &str,
    request: &Request<'_>,
) -> Result<RequestBuilder, ProviderError> {
    if client_id.trim().is_empty() || client_secret.trim().is_empty() {
        return Err(ProviderError::MissingKey);
    }
    let target = code(request.target)
        .ok_or_else(|| ProviderError::UnsupportedLanguage(request.target.to_owned()))?;
    let source = match request.source {
        Some(source) => {
            code(source).ok_or_else(|| ProviderError::UnsupportedLanguage(source.to_owned()))?
        }
        None => "auto",
    };
    Ok(client
        .post(URL)
        .header("X-NCP-APIGW-API-KEY-ID", client_id.trim())
        .header("X-NCP-APIGW-API-KEY", client_secret.trim())
        .form(&[
            ("source", source),
            ("target", target),
            ("text", request.text),
        ]))
}

pub(crate) fn parse(body: &str) -> Result<Translation, ProviderError> {
    let value: Value = serde_json::from_str(body).map_err(|_| ProviderError::BadResponse)?;
    let result = value
        .pointer("/message/result")
        .ok_or(ProviderError::BadResponse)?;
    let text = result
        .get("translatedText")
        .and_then(Value::as_str)
        .ok_or(ProviderError::BadResponse)?;
    let detected = result
        .get("srcLangType")
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
    fn rejects_unsupported_language_locally() {
        let client = test_client();
        let req = Request {
            text: "x",
            source: None,
            target: "pl",
        };
        assert!(matches!(
            request(&client, "id", "sec", &req),
            Err(ProviderError::UnsupportedLanguage(_))
        ));
    }

    #[test]
    fn builds_form_request_with_headers() {
        let client = test_client();
        let req = Request {
            text: "Hello",
            source: None,
            target: "ko",
        };
        assert!(matches!(
            request(&client, "id", "", &req),
            Err(ProviderError::MissingKey)
        ));
        let built = request(&client, "id", "sec", &req)
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(built.headers()["X-NCP-APIGW-API-KEY-ID"], "id");
        let body = std::str::from_utf8(built.body().unwrap().as_bytes().unwrap()).unwrap();
        assert_eq!(body, "source=auto&target=ko&text=Hello");
    }

    #[test]
    fn parses_response() {
        let body = r#"{"message":{"result":{"srcLangType":"en","tarLangType":"ko","translatedText":"안녕"}}}"#;
        let parsed = parse(body).unwrap();
        assert_eq!(parsed.text, "안녕");
        assert_eq!(parsed.detected.as_deref(), Some("en"));
    }
}
