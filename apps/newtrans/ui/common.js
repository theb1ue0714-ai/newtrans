"use strict";
const TAURI = window.__TAURI__;
const invoke = (cmd, args) => TAURI.core.invoke(cmd, args);
const listen = (event, handler) => TAURI.event.listen(event, handler);
const PROVIDER_LABEL = { google: "Google", papago: "Papago", deepl: "DeepL" };

function langName(languages, code) {
  if (!code) return "";
  const found = languages.find((l) => l.code.toLowerCase() === code.toLowerCase());
  return found ? found.name : code;
}

function errorMessage(error) {
  if (!error) return "";
  const kind = typeof error === "string" ? error : error.kind;
  const map = {
    missing_key: "API 키가 없습니다. 설정에서 키를 등록하세요.",
    invalid_key: "API 키가 올바르지 않습니다.",
    quota_exceeded: "이번 달 무료 사용량을 모두 사용했습니다.",
    rate_limited: "요청이 너무 많습니다. 잠시 후 다시 시도하세요.",
    unsupported_language: "이 서비스는 선택한 언어를 지원하지 않습니다.",
    timeout: "응답 시간이 초과되었습니다.",
    network: "네트워크에 연결할 수 없습니다.",
    upstream: `서비스 오류 (HTTP ${error.detail ?? ""})`,
    bad_response: "서비스 응답을 해석하지 못했습니다.",
  };
  return map[kind] || String(kind);
}

function speak(text, lang) {
  if (!text || !("speechSynthesis" in window)) return false;
  speechSynthesis.cancel();
  const utterance = new SpeechSynthesisUtterance(text);
  if (lang) utterance.lang = lang;
  speechSynthesis.speak(utterance);
  return true;
}
