export const WEBHOOK_CARD_URI = "ui://hooktry/webhook-card-v1.html";
export const WEBHOOK_CARD_MIME_TYPE = "text/html;profile=mcp-app";

export const WEBHOOK_CARD_HTML = String.raw`<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Hooktry webhook</title>
  <style>
    :root {
      color-scheme: light dark;
      font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
      --bg: #ffffff;
      --surface: #f7f7f8;
      --text: #151515;
      --muted: #6f6f73;
      --border: rgba(0, 0, 0, 0.10);
      --button: #111111;
      --button-text: #ffffff;
      --secondary: rgba(0, 0, 0, 0.06);
    }

    @media (prefers-color-scheme: dark) {
      :root {
        --bg: #161616;
        --surface: #202020;
        --text: #f5f5f5;
        --muted: #a5a5aa;
        --border: rgba(255, 255, 255, 0.12);
        --button: #f5f5f5;
        --button-text: #111111;
        --secondary: rgba(255, 255, 255, 0.08);
      }
    }

    * {
      box-sizing: border-box;
    }

    html,
    body {
      margin: 0;
      width: 100%;
      background: transparent;
      color: var(--text);
    }

    body {
      padding: 0;
    }

    button {
      font: inherit;
    }

    .card {
      width: 100%;
      max-width: 680px;
      margin: 0 auto;
      border: 1px solid var(--border);
      border-radius: 18px;
      background: var(--bg);
      overflow: hidden;
    }

    .header {
      display: flex;
      align-items: center;
      gap: 9px;
      padding: 14px 16px 11px;
    }

    .mark {
      width: 22px;
      height: 18px;
      flex: 0 0 auto;
    }

    .brand {
      font-size: 15px;
      font-weight: 650;
      letter-spacing: -0.01em;
    }

    .subtitle {
      margin-left: auto;
      font-size: 12px;
      color: var(--muted);
    }

    .group {
      padding: 0 16px 12px;
    }

    .capability {
      padding: 11px 0;
    }

    .capability + .capability {
      border-top: 1px solid var(--border);
    }

    .owner {
      border-top: 1px solid var(--border);
      background: var(--surface);
      padding: 13px 16px;
    }

    .row-head {
      display: flex;
      align-items: center;
      min-height: 28px;
      gap: 10px;
      margin-bottom: 5px;
    }

    .label-wrap {
      min-width: 0;
      display: flex;
      align-items: baseline;
      gap: 8px;
    }

    .label {
      font-size: 12px;
      line-height: 1.2;
      font-weight: 700;
      letter-spacing: 0.055em;
      text-transform: uppercase;
      color: var(--muted);
    }

    .badge {
      font-size: 11px;
      line-height: 1;
      font-weight: 650;
      color: var(--muted);
      white-space: nowrap;
    }

    .actions {
      display: flex;
      gap: 6px;
      margin-left: auto;
      flex: 0 0 auto;
    }

    .action {
      appearance: none;
      border: 0;
      border-radius: 9px;
      padding: 6px 9px;
      min-height: 30px;
      background: var(--secondary);
      color: var(--text);
      font-size: 12px;
      font-weight: 650;
      cursor: pointer;
    }

    .action.primary {
      background: var(--button);
      color: var(--button-text);
    }

    .action:active {
      transform: translateY(1px);
    }

    .url {
      display: block;
      width: 100%;
      margin: 0;
      color: var(--text);
      font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", monospace;
      font-size: clamp(11px, 3.05vw, 13px);
      line-height: 1.38;
      letter-spacing: -0.025em;
      overflow-wrap: anywhere;
      word-break: break-all;
      white-space: normal;
      user-select: text;
      -webkit-user-select: text;
      cursor: text;
    }

    .meta {
      display: grid;
      grid-template-columns: repeat(2, minmax(0, 1fr));
      border-top: 1px solid var(--border);
      padding: 11px 16px 13px;
      gap: 7px 14px;
      font-size: 12px;
      color: var(--muted);
    }

    .meta strong {
      color: var(--text);
      font-weight: 650;
    }

    .meta-item {
      min-width: 0;
    }

    .loading {
      padding: 18px 16px;
      color: var(--muted);
      font-size: 13px;
    }

    @media (max-width: 420px) {
      .card {
        border-radius: 16px;
      }

      .header {
        padding: 12px 14px 9px;
      }

      .group {
        padding: 0 14px 10px;
      }

      .owner {
        padding: 12px 14px;
      }

      .meta {
        padding: 10px 14px 12px;
      }

      .subtitle {
        display: none;
      }

      .action {
        padding-left: 8px;
        padding-right: 8px;
      }
    }

    @media (max-width: 340px) {
      .meta {
        grid-template-columns: 1fr;
      }

      .url {
        font-size: 10.5px;
      }
    }
  </style>
</head>
<body>
  <main id="card" class="card" hidden>
    <header class="header">
      <svg class="mark" viewBox="0 0 235 193" aria-hidden="true">
        <path d="M 184 6 L 108 6 L 6 134 L 60 134 C 66 134 70 131 74 126 L 102 90 C 109 81 115 76 125 76 L 130 76 C 136 76 141 72 145 67 Z" fill="currentColor"/>
        <path d="M 228 76 L 158 76 C 153 76 149 79 145 84 L 69 186 L 142 186 C 147 186 151 183 155 178 L 228 77 Z" fill="currentColor"/>
      </svg>
      <div class="brand">Hooktry</div>
      <div id="subtitle" class="subtitle">Temporary webhook</div>
    </header>

    <section class="group">
      <article class="capability">
        <div class="row-head">
          <div class="label-wrap">
            <div id="webhook-label" class="label">Webhook</div>
          </div>
          <div class="actions">
            <button id="copy-webhook" class="action" type="button">Copy</button>
          </div>
        </div>
        <code id="hook-url" class="url"></code>
      </article>

      <article class="capability">
        <div class="row-head">
          <div class="label-wrap">
            <div id="view-label" class="label">View</div>
          </div>
          <div class="actions">
            <button id="copy-view" class="action" type="button">Copy</button>
            <button id="open-view" class="action primary" type="button">Open</button>
          </div>
        </div>
        <code id="view-url" class="url"></code>
      </article>
    </section>

    <section class="owner">
      <div class="row-head">
        <div class="label-wrap">
          <div id="owner-label" class="label">Open as owner</div>
          <div id="owner-expiry" class="badge"></div>
        </div>
        <div class="actions">
          <button id="copy-owner" class="action" type="button">Copy</button>
          <button id="open-owner" class="action primary" type="button">Open</button>
        </div>
      </div>
      <code id="owner-url" class="url"></code>
    </section>

    <footer class="meta">
      <div id="requests-meta" class="meta-item"></div>
      <div id="body-meta" class="meta-item"></div>
      <div id="retained-meta" class="meta-item"></div>
      <div id="endpoint-expiry-meta" class="meta-item"></div>
    </footer>
  </main>

  <div id="loading" class="loading">Preparing webhook…</div>

  <script>
    (function () {
      var state = null;
      var countdownTimer = null;

      var strings = {
        en: {
          subtitle: "Temporary webhook",
          webhook: "Webhook",
          view: "View requests",
          owner: "Open as owner",
          copy: "Copy",
          copied: "Copied",
          open: "Open",
          requests: "requests",
          perRequest: "/request",
          retained: "retained",
          expires: "expires",
          ownerExpired: "expired",
          ownerMinutes: "min",
          ownerHours: "h",
          day: "day",
          days: "days",
          hour: "hour",
          hours: "hours",
          minute: "minute",
          minutes: "minutes"
        },
        uk: {
          subtitle: "Тимчасовий вебхук",
          webhook: "Webhook",
          view: "Перегляд запитів",
          owner: "Відкрити як власник",
          copy: "Копіювати",
          copied: "Скопійовано",
          open: "Відкрити",
          requests: "запитів",
          perRequest: "/запит",
          retained: "збережено",
          expires: "діє ще",
          ownerExpired: "прострочено",
          ownerMinutes: "хв",
          ownerHours: "год",
          day: "день",
          days: "днів",
          hour: "година",
          hours: "годин",
          minute: "хвилина",
          minutes: "хвилин"
        },
        ru: {
          subtitle: "Временный вебхук",
          webhook: "Webhook",
          view: "Просмотр запросов",
          owner: "Открыть как владелец",
          copy: "Копировать",
          copied: "Скопировано",
          open: "Открыть",
          requests: "запросов",
          perRequest: "/запрос",
          retained: "сохранено",
          expires: "действует ещё",
          ownerExpired: "истёк",
          ownerMinutes: "мин",
          ownerHours: "ч",
          day: "день",
          days: "дней",
          hour: "час",
          hours: "часов",
          minute: "минута",
          minutes: "минут"
        }
      };

      function localeKey() {
        var raw = "";
        try {
          raw = (window.openai && window.openai.locale) || document.documentElement.lang || navigator.language || "en";
        } catch (_) {
          raw = document.documentElement.lang || navigator.language || "en";
        }
        raw = String(raw).toLowerCase();
        if (raw.indexOf("uk") === 0) return "uk";
        if (raw.indexOf("ru") === 0) return "ru";
        return "en";
      }

      function t(key) {
        var dict = strings[localeKey()] || strings.en;
        return dict[key] || strings.en[key] || key;
      }

      function bytes(value) {
        var n = Number(value || 0);
        if (n >= 1024 * 1024 * 1024) return (n / (1024 * 1024 * 1024)).toFixed(n % (1024 * 1024 * 1024) === 0 ? 0 : 1) + " GB";
        if (n >= 1024 * 1024) return (n / (1024 * 1024)).toFixed(n % (1024 * 1024) === 0 ? 0 : 1) + " MB";
        if (n >= 1024) return (n / 1024).toFixed(n % 1024 === 0 ? 0 : 1) + " KB";
        return n + " B";
      }

      function relativeExpiry(unixSeconds) {
        if (!unixSeconds) return "";
        var seconds = Math.max(0, Number(unixSeconds) - Math.floor(Date.now() / 1000));
        if (seconds <= 0) return t("ownerExpired");
        var minutes = Math.ceil(seconds / 60);
        if (minutes < 60) return minutes + " " + t("ownerMinutes");
        var hours = Math.ceil(minutes / 60);
        if (hours < 24) return hours + " " + t("ownerHours");
        var days = Math.ceil(hours / 24);
        return days + " " + (days === 1 ? t("day") : t("days"));
      }

      function endpointExpiry(unixSeconds) {
        if (!unixSeconds) return "";
        var seconds = Math.max(0, Number(unixSeconds) - Math.floor(Date.now() / 1000));
        if (seconds <= 0) return t("ownerExpired");
        var minutes = Math.ceil(seconds / 60);
        if (minutes < 60) return minutes + " " + (minutes === 1 ? t("minute") : t("minutes"));
        var hours = Math.ceil(minutes / 60);
        if (hours < 24) return hours + " " + (hours === 1 ? t("hour") : t("hours"));
        var days = Math.ceil(hours / 24);
        return days + " " + (days === 1 ? t("day") : t("days"));
      }

      function notifyHeight() {
        try {
          if (window.openai && window.openai.notifyIntrinsicHeight) {
            window.openai.notifyIntrinsicHeight(document.documentElement.scrollHeight);
          }
        } catch (_) {}
      }

      function openExternal(href) {
        if (!href) return;
        try {
          if (window.openai && window.openai.openExternal) {
            window.openai.openExternal({ href: href, redirectUrl: false });
            return;
          }
        } catch (_) {}
        window.open(href, "_blank", "noopener,noreferrer");
      }

      async function copyText(value, button) {
        if (!value) return;
        try {
          if (navigator.clipboard && navigator.clipboard.writeText) {
            await navigator.clipboard.writeText(value);
          } else {
            var textarea = document.createElement("textarea");
            textarea.value = value;
            textarea.setAttribute("readonly", "");
            textarea.style.position = "fixed";
            textarea.style.opacity = "0";
            document.body.appendChild(textarea);
            textarea.select();
            document.execCommand("copy");
            textarea.remove();
          }
          var old = button.textContent;
          button.textContent = t("copied");
          window.setTimeout(function () {
            button.textContent = old;
          }, 1200);
        } catch (_) {}
      }

      function applyLabels() {
        document.getElementById("subtitle").textContent = t("subtitle");
        document.getElementById("webhook-label").textContent = t("webhook");
        document.getElementById("view-label").textContent = t("view");
        document.getElementById("owner-label").textContent = t("owner");
        ["copy-webhook", "copy-view", "copy-owner"].forEach(function (id) {
          document.getElementById(id).textContent = t("copy");
        });
        ["open-view", "open-owner"].forEach(function (id) {
          document.getElementById(id).textContent = t("open");
        });
      }

      function render() {
        if (!state || !state.hook_url) return;

        applyLabels();

        document.getElementById("hook-url").textContent = state.hook_url || "";
        document.getElementById("view-url").textContent = state.view_url || "";
        document.getElementById("owner-url").textContent = state.handoff_url || "";

        document.getElementById("requests-meta").innerHTML =
          "<strong>" + Number(state.request_limit || 0).toLocaleString() + "</strong> " + t("requests");
        document.getElementById("body-meta").innerHTML =
          "<strong>" + bytes(state.max_body_bytes) + "</strong>" + t("perRequest");
        document.getElementById("retained-meta").innerHTML =
          "<strong>" + bytes(state.max_retained_bytes) + "</strong> " + t("retained");
        document.getElementById("endpoint-expiry-meta").innerHTML =
          t("expires") + " <strong>" + endpointExpiry(state.expires_at_unix_seconds) + "</strong>";

        document.getElementById("owner-expiry").textContent =
          relativeExpiry(state.handoff_expires_at_unix_seconds);

        document.getElementById("card").hidden = false;
        document.getElementById("loading").hidden = true;

        if (countdownTimer) window.clearInterval(countdownTimer);
        countdownTimer = window.setInterval(function () {
          document.getElementById("owner-expiry").textContent =
            relativeExpiry(state.handoff_expires_at_unix_seconds);
          document.getElementById("endpoint-expiry-meta").innerHTML =
            t("expires") + " <strong>" + endpointExpiry(state.expires_at_unix_seconds) + "</strong>";
          notifyHeight();
        }, 30000);

        notifyHeight();
      }

      function updateFromResult(result) {
        if (!result) return;
        var next = result.structuredContent || result;
        if (!next || typeof next !== "object") return;
        if (!next.hook_url) return;
        state = next;
        render();
      }

      document.getElementById("copy-webhook").addEventListener("click", function () {
        copyText(state && state.hook_url, this);
      });
      document.getElementById("copy-view").addEventListener("click", function () {
        copyText(state && state.view_url, this);
      });
      document.getElementById("copy-owner").addEventListener("click", function () {
        copyText(state && state.handoff_url, this);
      });
      document.getElementById("open-view").addEventListener("click", function () {
        openExternal(state && state.view_url);
      });
      document.getElementById("open-owner").addEventListener("click", function () {
        openExternal(state && state.handoff_url);
      });

      var rpcId = 0;
      var pending = new Map();

      function rpcNotify(method, params) {
        window.parent.postMessage({ jsonrpc: "2.0", method: method, params: params }, "*");
      }

      function rpcRequest(method, params) {
        return new Promise(function (resolve, reject) {
          var id = ++rpcId;
          pending.set(id, { resolve: resolve, reject: reject });
          window.parent.postMessage({ jsonrpc: "2.0", id: id, method: method, params: params }, "*");
        });
      }

      window.addEventListener("message", function (event) {
        if (event.source !== window.parent) return;
        var message = event.data;
        if (!message || message.jsonrpc !== "2.0") return;

        if (typeof message.id === "number") {
          var request = pending.get(message.id);
          if (!request) return;
          pending.delete(message.id);
          if (message.error) request.reject(message.error);
          else request.resolve(message.result);
          return;
        }

        if (message.method === "ui/notifications/tool-result") {
          updateFromResult(message.params);
        }
      }, { passive: true });

      (async function initialize() {
        try {
          await rpcRequest("ui/initialize", {
            appInfo: { name: "hooktry-webhook-card", version: "1.0.0" },
            appCapabilities: {},
            protocolVersion: "2026-01-26"
          });
          rpcNotify("ui/notifications/initialized", {});
        } catch (_) {}

        try {
          if (window.openai && window.openai.toolOutput) {
            updateFromResult(window.openai.toolOutput);
          }
        } catch (_) {}

        applyLabels();
        notifyHeight();
      })();
    })();
  </script>
</body>
</html>`;
