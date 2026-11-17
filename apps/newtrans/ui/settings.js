"use strict";
const $ = (id) => document.getElementById(id);
let boot;
let draft;
let recording = null;

function hotkeyLabel(value) {
  if (!value) return "사용 안 함";
  const match = /^Double(\w+)$/.exec(value);
  return match ? `${match[1]} 두 번` : value;
}

function fillSelect(select, languages, extra) {
  select.replaceChildren(...(extra ? [new Option(extra[1], extra[0])] : []), ...languages.map((l) => new Option(l.name, l.code)));
}

function render() {
  const { languages } = boot;
  $("version").textContent = `버전 ${boot.version} · ${boot.platform}`;
  $("hk-primary").textContent = hotkeyLabel(draft.hotkeys.primary);
  $("hk-secondary").textContent = hotkeyLabel(draft.hotkeys.secondary);
  $("interval").value = draft.hotkeys.double_tap_interval_ms;

  const enabledLangs = languages.filter((l) => draft.languages.enabled.includes(l.code));
  fillSelect($("src"), enabledLangs, ["auto", "자동 감지"]);
  fillSelect($("target"), enabledLangs);
  fillSelect($("fallback"), enabledLangs);
  $("src").value = draft.languages.source.mode === "fixed" ? draft.languages.source.code : "auto";
  $("target").value = draft.languages.target;
  $("fallback").value = draft.languages.fallback;
  $("back").checked = draft.languages.back_translate;
  $("enabled").replaceChildren(
    ...languages.map((l) => {
      const label = document.createElement("label");
      const box = document.createElement("input");
      box.type = "checkbox";
      box.value = l.code;
      box.checked = draft.languages.enabled.includes(l.code);
      box.addEventListener("change", () => {
        const set = new Set(draft.languages.enabled);
        box.checked ? set.add(l.code) : set.delete(l.code);
        draft.languages.enabled = languages.map((x) => x.code).filter((c) => set.has(c));
        render();
      });
      label.append(box, ` ${l.name}`);
      return label;
    }),
  );

  $("providers").replaceChildren(
    ...["google", "papago", "deepl"].map((p) => {
      const label = document.createElement("label");
      const box = document.createElement("input");
      box.type = "checkbox";
      box.checked = draft.providers.order.includes(p);
      box.addEventListener("change", () => {
        const set = new Set(draft.providers.order);
        box.checked ? set.add(p) : set.delete(p);
        draft.providers.order = ["google", "papago", "deepl"].filter((x) => set.has(x));
        render();
      });
      label.append(box, ` ${PROVIDER_LABEL[p]}`);
      return label;
    }),
  );
  $("default-tab").replaceChildren(...draft.providers.order.map((p) => new Option(PROVIDER_LABEL[p], p)));
  $("default-tab").value = draft.providers.default_tab;
  $("google-mode").value = draft.providers.google_mode;
  $("timeout").value = draft.providers.request_timeout_secs;

  $("autostart").checked = draft.autostart;
  $("on-top").checked = draft.always_on_top;
  $("blur").checked = draft.hide_on_blur;
  $("join").checked = draft.join_wrapped_lines;

  renderKeyStatus(boot.secrets);
  $("diag").replaceChildren(...(boot.diagnostics.length ? boot.diagnostics : ["문제 없음"]).map((m) => Object.assign(document.createElement("li"), { textContent: m })));
}

function renderKeyStatus(status) {
  $("st-deepl").textContent = status.deepl ? "등록됨" : "미등록";
  $("st-papago").textContent = status.papago ? "등록됨" : "미등록";
  $("st-google").textContent = status.google ? "등록됨" : "미등록";
}

function collect() {
  const src = $("src").value;
  draft.hotkeys.double_tap_interval_ms = Number($("interval").value);
  draft.languages.source = src === "auto" ? { mode: "auto" } : { mode: "fixed", code: src };
  draft.languages.target = $("target").value;
  draft.languages.fallback = $("fallback").value;
  draft.languages.back_translate = $("back").checked;
  draft.providers.default_tab = $("default-tab").value || draft.providers.order[0];
  draft.providers.google_mode = $("google-mode").value;
  draft.providers.request_timeout_secs = Number($("timeout").value);
  draft.autostart = $("autostart").checked;
  draft.always_on_top = $("on-top").checked;
  draft.hide_on_blur = $("blur").checked;
  draft.join_wrapped_lines = $("join").checked;
}

// Hotkey recorder: a combination (≤3 keys) or a modifier tapped twice.
const MODIFIER_KEYS = { Control: "Ctrl", Alt: "Alt", Shift: "Shift", Meta: "Super" };
let tap = { key: null, at: 0, clean: true };

function startRecording(slot) {
  recording = slot;
  tap = { key: null, at: 0, clean: true };
  const button = $(`hk-${slot}`);
  button.classList.add("recording");
  button.textContent = "키를 누르세요… (Esc 취소)";
}

function stopRecording() {
  if (!recording) return;
  $(`hk-${recording}`).classList.remove("recording");
  recording = null;
  render();
}

async function accept(value) {
  const slot = recording;
  try {
    const normalized = await invoke("validate_hotkey", { value });
    draft.hotkeys[slot] = normalized;
    $("hk-message").textContent = "";
  } catch (error) {
    $("hk-message").textContent = String(error);
    $("hk-message").className = "error";
  }
  stopRecording();
}

function keyName(event) {
  if (/^Key[A-Z]$/.test(event.code)) return event.code.slice(3);
  if (/^Digit\d$/.test(event.code)) return event.code.slice(5);
  if (/^F\d{1,2}$/.test(event.key)) return event.key;
  const named = { " ": "Space", Enter: "Enter", Tab: "Tab", Insert: "Insert", Home: "Home", End: "End", PageUp: "PageUp", PageDown: "PageDown", Pause: "Pause" };
  return named[event.key] || event.key;
}

document.addEventListener("keydown", (event) => {
  if (!recording) return;
  event.preventDefault();
  if (event.key === "Escape") return stopRecording();
  if (MODIFIER_KEYS[event.key]) {
    if (!event.repeat) tap.clean = true;
    return;
  }
  tap.clean = false;
  const parts = [];
  if (event.ctrlKey) parts.push("Ctrl");
  if (event.altKey) parts.push("Alt");
  if (event.shiftKey) parts.push("Shift");
  if (event.metaKey) parts.push("Super");
  parts.push(keyName(event));
  accept(parts.join("+"));
});

document.addEventListener("keyup", (event) => {
  if (!recording || !MODIFIER_KEYS[event.key] || !tap.clean) return;
  const now = performance.now();
  const name = MODIFIER_KEYS[event.key];
  if (tap.key === name && now - tap.at <= Number($("interval").value || 350) + 150) {
    accept(`Double${name}`);
  } else {
    tap = { key: name, at: now, clean: true };
    $(`hk-${recording}`).textContent = `${name} 한 번 더… 또는 다른 키와 함께`;
  }
});

document.addEventListener("click", async (event) => {
  const el = event.target.closest("button");
  if (!el) return;
  if (el.dataset.slot) startRecording(el.dataset.slot);
  if (el.dataset.clear) { draft.hotkeys[el.dataset.clear] = null; render(); }
  if (el.dataset.default) { draft.hotkeys[el.dataset.default] = el.dataset.default === "primary" ? "DoubleCtrl" : "Ctrl+Q"; render(); }
  if (el.dataset.remove) {
    const patch = el.dataset.remove === "papago" ? { papago_client_id: "", papago_client_secret: "" } : { [el.dataset.remove]: "" };
    const status = await invoke("save_secrets", { patch });
    boot.secrets = status;
    renderKeyStatus(status);
    $("key-message").textContent = "키를 삭제했습니다.";
  }
  if (el.dataset.verify) {
    await saveKeys();
    const provider = el.dataset.verify;
    const message = $("key-message");
    message.textContent = `${PROVIDER_LABEL[provider]} 확인 중…`;
    message.className = "";
    try {
      const sample = await invoke("verify_provider", { provider });
      message.textContent = `${PROVIDER_LABEL[provider]} 정상 작동: "Good morning" → "${sample}"`;
      message.className = "ok";
    } catch (error) {
      message.textContent = `${PROVIDER_LABEL[provider]}: ${errorMessage(error)}`;
      message.className = "error";
    }
  }
});

async function saveKeys() {
  const value = (id) => ($(id).value.trim() ? $(id).value.trim() : null);
  const patch = {
    deepl_api_key: value("deepl"),
    papago_client_id: value("papago-id"),
    papago_client_secret: value("papago-secret"),
    google_api_key: value("google-key"),
  };
  if (Object.values(patch).every((v) => v === null)) return;
  boot.secrets = await invoke("save_secrets", { patch });
  ["deepl", "papago-id", "papago-secret", "google-key"].forEach((id) => ($(id).value = ""));
  renderKeyStatus(boot.secrets);
}

$("save").addEventListener("click", async () => {
  collect();
  const message = $("save-message");
  try {
    await saveKeys();
    const failures = await invoke("save_settings", { settings: draft });
    message.textContent = failures.length ? `저장했지만 등록하지 못한 단축키: ${failures.join(", ")}` : "저장했습니다.";
    message.className = failures.length ? "error" : "ok";
    boot = await invoke("bootstrap");
    draft = structuredClone(boot.settings);
    render();
  } catch (error) {
    message.textContent = String(error);
    message.className = "error";
  }
});
$("close").addEventListener("click", () => invoke("hide_window"));

(async () => {
  boot = await invoke("bootstrap");
  draft = structuredClone(boot.settings);
  render();
})();
