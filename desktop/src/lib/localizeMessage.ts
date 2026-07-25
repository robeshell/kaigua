import i18n from "../i18n";

/** Canonical i18n keys for known `err.*` codes (case-insensitive input). */
const ERR_CANONICAL: Record<string, string> = {
  "err.apikey": "err.apiKey",
  "err.forbidden": "err.forbidden",
  "err.ratelimit": "err.rateLimit",
  "err.connect": "err.connect",
};

/** Hard fallbacks so toasts never show raw keys if i18n lookup fails. */
const ERR_FALLBACK: Record<string, Record<string, string>> = {
  "zh-Hans": {
    "err.apikey": "API Key 无效或未填写，请到设置中检查",
    "err.forbidden": "接口拒绝访问（403），请检查 API Key 权限",
    "err.ratelimit": "请求过于频繁，请稍后再试",
    "err.connect":
      "无法连接刮削服务器。若使用代理：请打开「系统代理」或「TUN 模式」后重试",
  },
  en: {
    "err.apikey": "API key missing or invalid — check Settings",
    "err.forbidden": "Access denied (403) — check API key permissions",
    "err.ratelimit": "Too many requests — try again later",
    "err.connect":
      "Cannot reach scrape servers. If using a proxy/VPN, enable system proxy or TUN mode.",
  },
  ja: {
    "err.apikey": "API キーが無効または未設定です。設定を確認してください",
    "err.forbidden": "アクセス拒否（403）。API キー権限を確認してください",
    "err.ratelimit": "リクエストが多すぎます。しばらくしてから再試行してください",
    "err.connect":
      "スクレイプサーバーに接続できません。プロキシ利用時はシステムプロキシまたは TUN を有効にしてください。",
  },
};

function uiLang(): "zh-Hans" | "en" | "ja" {
  const lng = (i18n.language || "zh-Hans").toLowerCase();
  if (lng.startsWith("ja")) return "ja";
  if (lng.startsWith("en")) return "en";
  return "zh-Hans";
}

function extractErrKey(message: string): string | null {
  const embedded = message.match(/\berr\.[A-Za-z0-9_]+\b/i);
  if (embedded) return embedded[0];
  if (/^err\./i.test(message.trim())) return message.trim();
  return null;
}

/** Map stable `err.*` keys (and common wrappers) to the active UI locale. */
export function localizeUserMessage(message: string): string {
  const raw = String(message ?? "").trim();
  if (!raw) return raw;

  const candidate = extractErrKey(raw);
  if (!candidate) return message;

  const norm = candidate.toLowerCase();
  const canonical = ERR_CANONICAL[norm] ?? candidate;
  const lang = uiLang();

  let translated =
    i18n.t(canonical, { keySeparator: false, defaultValue: "" }) ||
    ERR_FALLBACK[lang]?.[norm] ||
    ERR_FALLBACK["zh-Hans"]?.[norm] ||
    "";

  if (!translated || translated === canonical || translated === candidate) {
    translated = ERR_FALLBACK[lang]?.[norm] || ERR_FALLBACK["zh-Hans"]?.[norm] || "";
  }
  if (!translated) return message;

  if (raw !== candidate) {
    return raw.replace(new RegExp(candidate.replace(".", "\\."), "i"), translated);
  }
  return translated;
}
