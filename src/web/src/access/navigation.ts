/** Only the server's local OAuth authorization endpoint may resume after login. */
export function safeAuthorizeNext(search: string): string | null {
  const value = new URLSearchParams(search).get("next");
  if (!value || !value.startsWith("/oauth/authorize?") || value.includes("\\") || value.includes("#") || /[\u0000-\u001f]/.test(value)) return null;
  const target = new URL(value, "https://local.invalid");
  return target.origin === "https://local.invalid" && target.pathname === "/oauth/authorize" && target.search ? target.pathname + target.search : null;
}

const providerErrors: Record<string, string> = {
  account_pending: "帳號等待管理員審核，核准後才能登入。",
  account_rejected: "帳號申請未獲核准，請聯絡管理員。",
  registration_closed: "目前未開放註冊，請聯絡管理員。",
  setup_required: "請先建立第一位管理員。",
  identity_not_linked: "這個外部帳號尚未綁定。請先以原帳號登入，再到連線與存取綁定。",
  identity_conflict: "這個外部帳號已綁定其他帳號，請改用目前帳號的登入方式。",
  state_invalid: "登入或綁定請求已過期，或不屬於目前瀏覽器。請重新開始。",
  provider_unavailable: "登入供應商暫時無法連線，請稍後重試或使用原帳號登入。",
  access_denied: "已取消外部帳號授權。仍可使用原帳號登入。",
  account_unavailable: "這個帳號目前無法使用，請聯絡管理員。",
  provider_disabled: "部署端尚未設定這個登入供應商，請使用原帳號登入。",
};
export function providerErrorMessage(search: string): string | null {
  const code = new URLSearchParams(search).get("access_error");
  return code && Object.hasOwn(providerErrors, code) ? providerErrors[code] : null;
}
