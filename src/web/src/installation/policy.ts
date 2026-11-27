export interface CollectionPolicy {
  consent_required: boolean; respect_dnt: boolean; auto_page: boolean;
  spa_mode: "history" | "hash" | "manual"; auto_click: boolean; auto_scroll: boolean;
  query_mode: "strip" | "allowlist" | "keep"; query_allowlist: string[];
  referrer_scope: "none" | "origin" | "full"; click_text: boolean;
  ignore_selectors: string[]; allowed_traits: string[] | null;
  legacy_email: boolean; require_signed_identity: boolean;
}
export const legacyPolicy: CollectionPolicy = {consent_required:false,respect_dnt:false,auto_page:true,spa_mode:"history",auto_click:true,auto_scroll:true,query_mode:"keep",query_allowlist:[],referrer_scope:"full",click_text:true,ignore_selectors:[],allowed_traits:null,legacy_email:true,require_signed_identity:false};
export const minimalPolicy: CollectionPolicy = {...legacyPolicy,consent_required:true,respect_dnt:true,auto_click:false,auto_scroll:false,query_mode:"strip",referrer_scope:"origin",click_text:false,allowed_traits:[]};
export function previewUrl(raw: string, policy: CollectionPolicy) {
  const url = new URL(raw); url.hash = "";
  if (policy.query_mode !== "keep") {
    const pairs = [...url.searchParams].filter(([key]) => policy.query_mode === "allowlist" && policy.query_allowlist.includes(key));
    url.search = new URLSearchParams(pairs).toString();
  }
  return url.href;
}
