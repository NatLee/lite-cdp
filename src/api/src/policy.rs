//! Public, versioned collection settings. Legacy defaults preserve old embeds.
use crate::ingest::Payload;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CollectionPolicy {
    pub consent_required: bool,
    pub respect_dnt: bool,
    pub auto_page: bool,
    pub spa_mode: String,
    pub auto_click: bool,
    pub auto_scroll: bool,
    pub query_mode: String,
    pub query_allowlist: Vec<String>,
    pub referrer_scope: String,
    pub click_text: bool,
    pub ignore_selectors: Vec<String>,
    pub allowed_traits: Option<Vec<String>>,
    pub legacy_email: bool,
    pub require_signed_identity: bool,
}
impl Default for CollectionPolicy {
    fn default() -> Self {
        Self {
            consent_required: false,
            respect_dnt: false,
            auto_page: true,
            spa_mode: "history".into(),
            auto_click: true,
            auto_scroll: true,
            query_mode: "keep".into(),
            query_allowlist: vec![],
            referrer_scope: "full".into(),
            click_text: true,
            ignore_selectors: vec![],
            allowed_traits: None,
            legacy_email: true,
            require_signed_identity: false,
        }
    }
}
impl CollectionPolicy {
    pub fn parse(raw: &str) -> Self {
        serde_json::from_str(raw).unwrap_or_default()
    }
    pub fn validate(&self) -> Result<(), String> {
        if !["history", "hash", "manual"].contains(&self.spa_mode.as_str())
            || !["strip", "allowlist", "keep"].contains(&self.query_mode.as_str())
            || !["none", "origin", "full"].contains(&self.referrer_scope.as_str())
        {
            return Err("不支援的採集模式".into());
        }
        for values in [
            &self.query_allowlist,
            &self.ignore_selectors,
            self.allowed_traits
                .as_ref()
                .unwrap_or(&self.query_allowlist),
        ] {
            if values.len() > 30
                || values
                    .iter()
                    .any(|v| v.trim().is_empty() || v.len() > 120 || v.contains(char::is_control))
            {
                return Err("採集清單最多 30 個項目，每項 1–120 字元".into());
            }
        }
        Ok(())
    }
    pub fn url(&self, raw: &str, scope: &str) -> String {
        if raw.is_empty() || scope == "none" {
            return String::new();
        }
        let Ok(mut url) = url::Url::parse(raw) else {
            return String::new();
        };
        if !["http", "https"].contains(&url.scheme()) {
            return String::new();
        }
        if scope == "origin" {
            return url.origin().ascii_serialization();
        }
        url.set_fragment(None);
        let _ = url.set_username("");
        let _ = url.set_password(None);
        if self.query_mode != "keep" {
            let pairs: Vec<(String, String)> = if self.query_mode == "allowlist" {
                url.query_pairs()
                    .filter(|(k, _)| self.query_allowlist.iter().any(|a| a == k))
                    .map(|(k, v)| (k.into_owned(), v.into_owned()))
                    .collect()
            } else {
                vec![]
            };
            url.set_query(None);
            if !pairs.is_empty() {
                url.query_pairs_mut().extend_pairs(pairs);
            }
        }
        url.to_string()
    }
    pub fn apply(&self, payload: &mut Payload) {
        payload.events.retain(|e| match e.kind.as_str() {
            "click" => self.auto_click,
            "scroll" => self.auto_scroll,
            _ => true,
        });
        for event in &mut payload.events {
            if let Some(url) = &event.url {
                event.url = Some(self.url(url, "full"));
            }
            if let Some(url) = &event.referrer {
                event.referrer = Some(self.url(url, &self.referrer_scope));
            }
            if event.kind == "click" {
                if !self.click_text {
                    event.name = Some("click".into());
                }
                if let Some(serde_json::Value::Object(props)) = &mut event.props {
                    if let Some(serde_json::Value::String(href)) = props.get("href") {
                        let safe = self.url(href, "full");
                        props.insert("href".into(), safe.into());
                    }
                }
            }
        }
        if let Some(who) = &mut payload.identify {
            if let (Some(allowed), Some(traits)) = (&self.allowed_traits, &mut who.traits) {
                traits.retain(|key, _| allowed.contains(key));
            }
            if !self.legacy_email && who.user_id.as_deref().unwrap_or("").trim().is_empty() {
                payload.identify = None;
            }
        }
    }
}
