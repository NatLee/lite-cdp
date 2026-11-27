export const frameworks = ["HTML", "React / Vite", "Next.js", "Vue / Vite", "Nuxt 3", "Nuxt 4", "GTM"] as const;
export type Framework = typeof frameworks[number];
export function installationTemplate(framework: Framework, origin: string, key: string, testToken = "") {
  const src = `${origin}/t.js`;
  const attrs = `data-key="${key}"${testToken ? ` data-test-token="${testToken}"` : ""}`;
  const html = `<script async src="${src}" ${attrs}></script>`;
  if (framework === "HTML" || framework === "GTM") return html;
  if (framework === "React / Vite" || framework === "Vue / Vite") return `<!-- index.html：共用 head，整個應用只放一次 -->\n${html}`;
  if (framework === "Next.js") return `// app/layout.tsx — 加入既有 root layout 的 body\nimport Script from "next/script";\n\n<Script id="lite-cdp" src="${src}"\n  ${attrs} strategy="afterInteractive" />`;
  const directory = framework === "Nuxt 4" ? "app/plugins" : "plugins";
  return `// ${directory}/lite-cdp.client.ts\nexport default defineNuxtPlugin((app) => {\n  app.hook("app:mounted", () => {\n    const script = document.createElement("script");\n    script.src = ${JSON.stringify(src)};\n    script.async = true;\n    script.dataset.key = ${JSON.stringify(key)};${testToken ? `\n    script.dataset.testToken = ${JSON.stringify(testToken)};` : ""}\n    document.head.appendChild(script);\n  });\n});`;
}
export const lifecycleTemplate = `// 放在瀏覽器端的 CMP / 登入 / 登出 callback。不要在 render 中送事件。
const send = (...command) => {
  window.cdpq = window.cdpq || [];
  window.cdpq.push(command);
};

// CMP 在每次頁面載入及選擇變更時呼叫。拒絕後不回補歷史事件。
function onAnalyticsConsent(granted) {
  send("ready", (tracker) => tracker.consent(granted === true));
}

// 從你已驗證的登入結果取得穩定會員 ID；email 不當作新會員主鍵。
function onLogin(user) {
  send("ready", (tracker) => {
    tracker.identify({ user_id: String(user.id), email: user.email });
  });
}
function onLogout() { send("ready", (tracker) => tracker.reset()); }
function onPurchase(order) {
  send("track", "purchase", { order_id: order.id, total: order.total });
}`;
export const frameworkNotes: Record<Framework, string> = {
  HTML: "多頁網站放在共用版型的 head。第一方 ID 儲存在宿主網站，不是 CDP 伺服器網域。",
  "React / Vite": "放 index.html 可避開 component 重掛載。StrictMode 開發環境會重跑 effect；不要每次 mount 注入或每次 render 送事件。若必須動態載入，仍共用這一個全站 loader。",
  "Next.js": "放 root layout；next/script 會轉交 data-*。window、登入與 CMP callback 放 Client Component。不要在 server render 讀取瀏覽器儲存。",
  "Vue / Vite": "放 index.html；若使用動態 loader，放 App 根節點 onMounted。SSR 不在 setup 模組頂層讀取 window。",
  "Nuxt 3": "Nuxt 3 使用 plugins/*.client.ts；.client 保證只在瀏覽器執行。app:mounted 之後載入，CMP 仍需通知同意結果。",
  "Nuxt 4": "Nuxt 4 使用 app/plugins/*.client.ts。不要同時在 nuxt.config、插件與頁面重複加入。",
  GTM: "Custom HTML 設為 Once per page，要求 analytics_storage granted，並在 CMP granted 事件觸發。不要加 History Change 重載 tag。Consent Initialization 只給管理同意狀態的 CMP；撤回時仍要呼叫 cdp.consent(false)。",
};
