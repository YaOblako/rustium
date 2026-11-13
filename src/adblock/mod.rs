use adblock::lists::{FilterSet, ParseOptions};
use adblock::request::Request;
use adblock::Engine;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Mutex, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScamWarningInfo {
    pub url: String,
    pub domain: String,
    pub threat_type: String,
    pub list_source: String,
}

#[allow(dead_code)]
pub struct AdblockManager {
    engine: RwLock<Engine>,
    enabled: RwLock<bool>,
    stats_total_blocked: Mutex<u64>,
    stats_domain_blocked: Mutex<HashMap<String, u64>>,
    stats_tab_blocked: Mutex<HashMap<String, u64>>,
}

const DEFAULT_EASYLIST_RULES: &str = r#"
! -------------------------------------------------------------
! Rustium Built-in EasyList, uBlock Origin & Privacy Rules
! -------------------------------------------------------------
||doubleclick.net^
||googlesyndication.com^
||google-analytics.com^
||googleadservices.com^
||adservice.google.com^
||an.yandex.ru^
||mc.yandex.ru^
||ads.adfox.ru^
||awaps.yandex.ru^
||criteo.com^
||criteo.net^
||taboola.com^
||outbrain.com^
||scorecardresearch.com^
||adnxs.com^
||adroll.com^
||moatads.com^
||quantserve.com^
||rubiconproject.com^
||pubmatic.com^
||smartadserver.com^
||popads.net^
||popcash.net^
||propellerads.com^
||adsterra.com^
||zergnet.com^
||mgid.com^
||revcontent.com^
||bidswitch.net^
||hotjar.com^
||clarity.ms^
||top-fwz1.mail.ru^
! uBlock Origin Badware & Malware domains
||badware.org^
||malwaredomainlist.com^
||crypto-drainer.io^
||claim-airdrop-bonus.xyz^
||discord-nitro-gift.ru^
||steamcommunitly.com^
||steamconmunity.com^
||free-robux-generator.ru^
||wallet-connect-drain.net^
/ads/*
/adbanner/*
/advertisement/*
/popunder/*
/tracking.js
/analytics.js
/pagead/*
/ad-banner/*
/sponsor-*
/adsystem/*
/adserver/*
/telemetry/*
! Cosmetic Filters (Element Hiding)
##.ad-banner
##.adsbygoogle
##.ad-unit
##.ad-container
##.sponsor-banner
##.advertisement-box
##[id^="ad-"]
##[id^="google_ads_"]
##[class*="SponsoredContent"]
##.yandex-direct
##.ya-direct
##.direct-ad
"#;

#[allow(dead_code)]
impl AdblockManager {
    pub fn new(enabled: bool) -> Self {
        let mut filter_set = FilterSet::new(true);
        filter_set.add_filter_list(DEFAULT_EASYLIST_RULES.to_string(), ParseOptions::default());
        let engine = Engine::new_with_filter_set(filter_set);

        Self {
            engine: RwLock::new(engine),
            enabled: RwLock::new(enabled),
            stats_total_blocked: Mutex::new(0),
            stats_domain_blocked: Mutex::new(HashMap::new()),
            stats_tab_blocked: Mutex::new(HashMap::new()),
        }
    }

    pub fn is_enabled(&self) -> bool {
        *self.enabled.read().unwrap()
    }

    pub fn set_enabled(&self, val: bool) {
        *self.enabled.write().unwrap() = val;
    }

    pub fn check_scam_or_malware(&self, url: &str) -> Option<ScamWarningInfo> {
        if !self.is_enabled() {
            return None;
        }

        let lower = url.to_lowercase();
        let is_suspicious = lower.contains("phish")
            || lower.contains("crypto-drain")
            || lower.contains("steamcommunitly")
            || lower.contains("steamconmunity")
            || lower.contains("discord-nitro-gift")
            || lower.contains("claim-airdrop-bonus")
            || lower.contains("free-robux-generator")
            || lower.contains("wallet-connect-drain")
            || lower.contains("malwaredomainlist");

        if is_suspicious {
            let domain = url::Url::parse(url)
                .ok()
                .and_then(|u| u.host_str().map(|s| s.to_string()))
                .unwrap_or_else(|| url.to_string());

            return Some(ScamWarningInfo {
                url: url.to_string(),
                domain,
                threat_type: "Вредоносный или фишинговый ресурс (uBlock Badware)".to_string(),
                list_source: "Списки защиты uBlock Origin Anti-Phishing & Malware Protection".to_string(),
            });
        }

        None
    }

    pub fn add_custom_rules(&self, rules: &str) {
        let mut filter_set = FilterSet::new(true);
        filter_set.add_filter_list(DEFAULT_EASYLIST_RULES.to_string(), ParseOptions::default());
        filter_set.add_filter_list(rules.to_string(), ParseOptions::default());
        let new_engine = Engine::new_with_filter_set(filter_set);
        *self.engine.write().unwrap() = new_engine;
    }

    pub fn check_request(
        &self,
        url: &str,
        source_url: &str,
        req_type: &str,
        source_domain: &str,
        tab_id: Option<&str>,
    ) -> bool {
        if !self.is_enabled() {
            return false;
        }

        // Internal rustium protocol requests are always allowed
        if url.starts_with("rustium://") || source_url.starts_with("rustium://") {
            return false;
        }

        let engine = self.engine.read().unwrap();
        if let Ok(req) = Request::new(url, source_url, req_type, source_domain) {
            let res = engine.check_network_request(&req);
            let blocked = res.filter.is_some() && res.exception.is_none();
            if blocked {
                // Increment stats
                let mut total = self.stats_total_blocked.lock().unwrap();
                *total += 1;

                if !source_domain.is_empty() {
                    let mut domain_map = self.stats_domain_blocked.lock().unwrap();
                    *domain_map.entry(source_domain.to_string()).or_insert(0) += 1;
                }

                if let Some(tid) = tab_id {
                    let mut tab_map = self.stats_tab_blocked.lock().unwrap();
                    *tab_map.entry(tid.to_string()).or_insert(0) += 1;
                }

                return true;
            }
        }
        false
    }

    pub fn get_cosmetic_css(&self, url: &str) -> String {
        if !self.is_enabled() {
            return String::new();
        }

        let engine = self.engine.read().unwrap();
        let cosmetic = engine.url_cosmetic_resources(url);

        let mut css = String::new();
        if !cosmetic.hide_selectors.is_empty() {
            let selectors: Vec<&str> = cosmetic.hide_selectors.iter().map(|s| s.as_str()).collect();
            css.push_str(&format!(
                "{} {{ display: none !important; visibility: hidden !important; height: 0 !important; }}\n",
                selectors.join(", ")
            ));
        }

        css
    }

    pub fn get_tab_blocked_count(&self, tab_id: &str) -> u64 {
        let tab_map = self.stats_tab_blocked.lock().unwrap();
        tab_map.get(tab_id).copied().unwrap_or(0)
    }

    pub fn reset_tab_blocked_count(&self, tab_id: &str) {
        let mut tab_map = self.stats_tab_blocked.lock().unwrap();
        tab_map.remove(tab_id);
    }

    pub fn get_total_blocked_count(&self) -> u64 {
        *self.stats_total_blocked.lock().unwrap()
    }

    pub fn get_domain_blocked_count(&self, domain: &str) -> u64 {
        let domain_map = self.stats_domain_blocked.lock().unwrap();
        domain_map.get(domain).copied().unwrap_or(0)
    }

    pub fn get_injected_script(&self, url: &str, tab_id: &str) -> String {
        let cosmetic_css = self.get_cosmetic_css(url);
        format!(
            r#"(function() {{
                if (window.__rustium_adblock_injected__) return;
                window.__rustium_adblock_injected__ = true;
                const tabId = "{tab_id}";

                // 1. Inject Cosmetic CSS Rules
                const style = document.createElement("style");
                style.id = "rustium-adblock-cosmetic";
                style.textContent = `{cosmetic_css}
                    .ad-banner, .adsbygoogle, [class*="ad-box"], [id*="google_ads"], .yandex-direct {{
                        display: none !important;
                    }}
                `;
                (document.head || document.documentElement).appendChild(style);

                // 2. Intercept fetch & XMLHttpRequest
                const originalFetch = window.fetch;
                window.fetch = function(...args) {{
                    const reqUrl = typeof args[0] === 'string' ? args[0] : (args[0] && args[0].url ? args[0].url : '');
                    if (/doubleclick\.net|google-analytics\.com|adservice\.google|an\.yandex\.ru|mc\.yandex\.ru|taboola|outbrain|criteo/i.test(reqUrl)) {{
                        if (window.ipc) {{
                            window.ipc.postMessage(JSON.stringify({{ action: "ad_blocked", tab_id: tabId, url: reqUrl }}));
                        }}
                        return Promise.resolve(new Response('', {{ status: 204, statusText: 'Rustium Adblock Blocked' }}));
                    }}
                    return originalFetch.apply(this, args);
                }};

                const originalXhrOpen = XMLHttpRequest.prototype.open;
                XMLHttpRequest.prototype.open = function(method, reqUrl, ...rest) {{
                    if (typeof reqUrl === 'string' && /doubleclick\.net|google-analytics\.com|adservice\.google|an\.yandex\.ru|mc\.yandex\.ru|taboola|outbrain|criteo/i.test(reqUrl)) {{
                        if (window.ipc) {{
                            window.ipc.postMessage(JSON.stringify({{ action: "ad_blocked", tab_id: tabId, url: reqUrl }}));
                        }}
                        this.abort();
                        return;
                    }}
                    return originalXhrOpen.call(this, method, reqUrl, ...rest);
                }};

                // 3. MutationObserver for dynamic ads
                const observer = new MutationObserver(() => {{
                    const ads = document.querySelectorAll('.ad-banner, .adsbygoogle, [class*="ad-box"], [id*="google_ads"], .yandex-direct, iframe[src*="ad"]');
                    ads.forEach(el => {{
                        if (el.style.display !== 'none') {{
                            el.style.setProperty('display', 'none', 'important');
                        }}
                    }});
                }});
                observer.observe(document.documentElement, {{ childList: true, subtree: true }});
            }})();"#
        )
    }
}
