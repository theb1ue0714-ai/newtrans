"use strict";
const $ = (id) => document.getElementById(id);
const state = { boot: null, providers: [], active: null, cache: new Map(), requestId: 0, results: {} };

async function load() {
  state.boot = await invoke("bootstrap");
  const { settings } = state.boot;
  state.providers = settings.providers.order;
  if (!state.active || !state.providers.includes(state.active)) state.active = settings.providers.default_tab;
  renderTabs();
  renderLanguages();
}

function renderTabs() {
  const tabs = $("tabs");
  tabs.replaceChildren();
  state.providers.forEach((provider, index) => {
    const button = document.createElement("button");
    button.className = "tab";
    button.role = "tab";
    button.id = `tab-${provider}`;
    button.setAttribute("aria-selected", String(provider === state.active));
    button.setAttribute("aria-controls", "panel");
    button.tabIndex = provider === state.active ? 0 : -1;
    button.append(PROVIDER_LABEL[provider]);
    const key = document.createElement("span");
    key.className = "key";
    key.textContent = `Alt+${index + 1}`;
    button.append(key);
    button.addEventListener("click", () => selectTab(provider));
    tabs.append(button);
  });
}

function renderLanguages() {
  const { settings, languages } = state.boot;
  const enabled = languages.filter((l) => settings.languages.enabled.includes(l.code));
  const lang1 = $("lang1");
  const lang2 = $("lang2");
  lang1.replaceChildren(new Option("자동 감지", "auto"), ...enabled.map((l) => new Option(l.name, l.code)));
  lang2.replaceChildren(...enabled.map((l) => new Option(l.name, l.code)));
  const source = settings.languages.source;
  lang1.value = source.mode === "fixed" ? source.code : "auto";
  lang2.value = settings.languages.target;
}

function currentChoice() {
  const value = $("lang1").value;
  return value === "auto" ? { mode: "auto" } : { mode: "fixed", code: value };
}

function cacheKey(provider, text) {
  return [provider, text, $("lang1").value, $("lang2").value].join("\u0001");
}

function selectTab(provider) {
  if (!state.providers.includes(provider)) return;
  state.active = provider;
  document.querySelectorAll(".tab").forEach((tab) => {
    const selected = tab.id === `tab-${provider}`;
    tab.setAttribute("aria-selected", String(selected));
    tab.tabIndex = selected ? 0 : -1;
  });
  showActive();
  ensure(provider);
}

function cycle(step) {
  const index = state.providers.indexOf(state.active);
  const next = state.providers[(index + step + state.providers.length) % state.providers.length];
  selectTab(next);
  document.getElementById(`tab-${next}`)?.focus();
}

function ensure(provider) {
  const text = $("source").value.trim();
  if (!text) return;
  const key = cacheKey(provider, text);
  if (state.cache.has(key)) return;
  const entry = { status: "loading" };
  state.cache.set(key, entry);
  invoke("translate", { provider, text, source: currentChoice(), target: $("lang2").value })
    .then((result) => Object.assign(entry, { status: "done", result }))
    .catch((error) => Object.assign(entry, { status: "error", error }))
    .finally(() => {
      if (provider === state.active && cacheKey(provider, $("source").value.trim()) === key) showActive();
    });
  if (provider === state.active) showActive();
}

function translateAll() {
  // Active tab first, then the others in the background so switching tabs
  // is instant.
  ensure(state.active);
  state.providers.filter((p) => p !== state.active).forEach(ensure);
}

function showActive() {
  const { languages } = state.boot;
  const text = $("source").value.trim();
  const entry = text ? state.cache.get(cacheKey(state.active, text)) : null;
  const translation = $("translation");
  const back = $("back");
  translation.classList.toggle("loading", entry?.status === "loading");
  translation.classList.remove("error");
  back.classList.remove("error");
  $("status").textContent = "";
  state.results = {};
  $("detected").textContent = "";
  $("mid-lang").textContent = $("back-lang").textContent = $("src-lang").textContent = "";
  if (!entry || entry.status === "loading") {
    translation.textContent = "";
    back.textContent = "";
    return;
  }
  if (entry.status === "error") {
    translation.textContent = errorMessage(entry.error);
    translation.classList.add("error");
    back.textContent = "";
    if (entry.error?.kind === "missing_key") $("status").textContent = `${PROVIDER_LABEL[state.active]} 키는 설정 → API 키에서 등록합니다.`;
    return;
  }
  const r = entry.result;
  state.results = { translation: r.translation, back: r.back_translation, srcLang: r.source_lang, midLang: r.target_lang, backLang: r.back_lang };
  translation.textContent = r.translation;
  $("src-lang").textContent = r.source_lang ? `· ${langName(languages, r.source_lang)}` : "";
  $("mid-lang").textContent = `· ${langName(languages, r.target_lang)}`;
  $("back-lang").textContent = r.back_lang ? `· ${langName(languages, r.back_lang)}` : "";
  $("detected").textContent = $("lang1").value === "auto" && r.source_lang ? `감지: ${langName(languages, r.source_lang)}` : "";
  if (r.back_error) {
    back.textContent = errorMessage(r.back_error);
    back.classList.add("error");
  } else {
    back.textContent = r.back_translation ?? (r.back_lang ? "" : "재번역 없음 (원문과 번역 언어가 같거나 꺼짐)");
  }
}

function setText(text) {
  $("source").value = text || "";
  showActive();
  translateAll();
}

async function pinLanguages() {
  const target = $("lang2").value;
  try {
    await invoke("pin_languages", { source: currentChoice(), target });
    state.boot = await invoke("bootstrap");
  } catch (error) {
    $("status").textContent = String(error);
  }
  showActive();
  translateAll();
}

function swapLanguages() {
  const detected = state.results.srcLang;
  const from = $("lang1").value === "auto" ? detected : $("lang1").value;
  if (!from) return;
  const to = $("lang2").value;
  $("lang1").value = to;
  $("lang2").value = from;
  if (state.results.translation) $("source").value = state.results.translation;
  pinLanguages();
}

function valueOf(which) {
  if (which === "source") return { text: $("source").value, lang: state.results.srcLang };
  if (which === "translation") return { text: state.results.translation, lang: state.results.midLang };
  return { text: state.results.back, lang: state.results.backLang };
}

document.addEventListener("click", async (event) => {
  const speakTarget = event.target.closest("[data-speak]")?.dataset.speak;
  const copyTarget = event.target.closest("[data-copy]")?.dataset.copy;
  if (speakTarget) {
    const { text, lang } = valueOf(speakTarget);
    if (!speak(text, lang)) $("status").textContent = "이 시스템에서는 음성 읽기를 사용할 수 없습니다.";
  } else if (copyTarget) {
    const { text } = valueOf(copyTarget);
    if (text) {
      await navigator.clipboard.writeText(text).catch(() => {});
      $("status").textContent = "복사했습니다.";
    }
  }
});

document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") {
    invoke("hide_window");
  } else if (event.ctrlKey && event.key === "Tab") {
    event.preventDefault();
    cycle(event.shiftKey ? -1 : 1);
  } else if (event.altKey && /^[1-9]$/.test(event.key)) {
    event.preventDefault();
    selectTab(state.providers[Number(event.key) - 1]);
  } else if ((event.ctrlKey || event.metaKey) && event.key === "Enter") {
    event.preventDefault();
    translateAll();
  } else if ((event.ctrlKey || event.metaKey) && (event.key === "i" || event.key === "I")) {
    event.preventDefault();
    swapLanguages();
  } else if (event.target.role === "tab" && (event.key === "ArrowRight" || event.key === "ArrowLeft")) {
    event.preventDefault();
    cycle(event.key === "ArrowRight" ? 1 : -1);
  }
});

$("lang1").addEventListener("change", pinLanguages);
$("lang2").addEventListener("change", pinLanguages);
$("swap-btn").addEventListener("click", swapLanguages);
$("translate-btn").addEventListener("click", translateAll);
$("settings-btn").addEventListener("click", () => invoke("open_settings"));
$("close-btn").addEventListener("click", () => invoke("hide_window"));

(async () => {
  await load();
  listen("new-text", ({ payload }) => {
    if (payload) setText(payload);
    else {
      $("status").textContent = "선택된 텍스트를 찾지 못했습니다. 직접 입력하세요.";
      $("source").focus();
    }
    document.getElementById(`tab-${state.active}`)?.focus();
  });
  listen("cycle-tab", () => cycle(1));
  listen("settings-changed", async () => {
    state.cache.clear();
    await load();
    showActive();
  });
  const pending = await invoke("take_pending_text");
  if (pending) setText(pending);
})();
