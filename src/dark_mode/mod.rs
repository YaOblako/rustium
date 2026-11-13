use std::sync::RwLock;

pub struct DarkModeEngine {
    mode: RwLock<String>, // "system", "always_on", "always_off"
    brightness: RwLock<u32>,
    contrast: RwLock<u32>,
}

#[allow(dead_code)]
impl DarkModeEngine {
    pub fn new(mode: &str, brightness: u32, contrast: u32) -> Self {
        Self {
            mode: RwLock::new(mode.to_string()),
            brightness: RwLock::new(brightness),
            contrast: RwLock::new(contrast),
        }
    }

    pub fn set_mode(&self, mode: &str) {
        *self.mode.write().unwrap() = mode.to_string();
    }

    pub fn set_brightness(&self, val: u32) {
        *self.brightness.write().unwrap() = val;
    }

    pub fn set_contrast(&self, val: u32) {
        *self.contrast.write().unwrap() = val;
    }

    pub fn get_mode(&self) -> String {
        self.mode.read().unwrap().clone()
    }

    pub fn get_brightness(&self) -> u32 {
        *self.brightness.read().unwrap()
    }

    pub fn get_contrast(&self) -> u32 {
        *self.contrast.read().unwrap()
    }

    pub fn should_apply_to(&self, _domain: &str, is_whitelisted: bool) -> bool {
        if is_whitelisted {
            return false;
        }

        let mode = self.get_mode();
        match mode.as_str() {
            "always_on" => true,
            "always_off" => false,
            "system" | _ => true,
        }
    }

    pub fn get_injected_script(&self, is_enabled_for_page: bool) -> String {
        let brightness = self.get_brightness();
        let contrast = self.get_contrast();
        let enabled = if is_enabled_for_page { "true" } else { "false" };
        let b = (brightness as f32) / 100.0;
        let c = (contrast as f32) / 100.0;

        format!(
            r#"(function() {{
                if (window.__rustium_dark_mode__) {{
                    window.__rustium_dark_mode__.updateConfig({{
                        enabled: {enabled},
                        brightness: {brightness},
                        contrast: {contrast}
                    }});
                    return;
                }}

                class RustiumDarkReader {{
                    constructor(config) {{
                        this.config = config;
                        this.styleEl = null;
                        this.init();
                    }}

                    init() {{
                        this.defaultCss = 'html[data-rustium-dark="true"] {{ filter: invert(90%) hue-rotate(180deg) brightness({b}) contrast({c}) !important; }}';
                        this.createStyleElement();
                        if (document.readyState === 'loading') {{
                            document.addEventListener('DOMContentLoaded', () => this.apply());
                        }} else {{
                            this.apply();
                        }}
                    }}

                    createStyleElement() {{
                        if (!this.styleEl) {{
                            this.styleEl = document.createElement('style');
                            this.styleEl.id = 'rustium-dark-reader-style';
                            const target = document.head || document.documentElement;
                            if (target) {{
                                target.appendChild(this.styleEl);
                            }}
                        }}
                    }}

                    isAlreadyDark() {{
                        try {{
                            // 1. Check meta color-scheme
                            const meta = document.querySelector('meta[name="color-scheme"]');
                            if (meta && meta.content && meta.content.includes('dark')) return true;

                            // 2. Check documentElement or body background luminance
                            const el = document.body || document.documentElement;
                            if (!el) return false;
                            const bg = window.getComputedStyle(el).backgroundColor;
                            const match = bg.match(/rgba?\((\d+),\s*(\d+),\s*(\d+)/);
                            if (match) {{
                                const r = parseInt(match[1]);
                                const g = parseInt(match[2]);
                                const b = parseInt(match[3]);
                                const lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
                                if (lum < 55) return true; // Dark website detected!
                            }}

                            // 3. Known sites with dark mode enabled by default
                            const host = window.location.hostname;
                            if (host.includes('discord.com')) return true;
                            if (host.includes('youtube.com') && document.documentElement.hasAttribute('dark')) return true;
                            if (host.includes('github.com') && document.documentElement.getAttribute('data-color-mode') === 'dark') return true;
                        }} catch (e) {{}}
                        return false;
                    }}

                    updateConfig(newConfig) {{
                        this.config = Object.assign(this.config, newConfig);
                        this.apply();
                    }}

                    apply() {{
                        this.createStyleElement();
                        if (!this.styleEl) return;

                        if (!this.config.enabled || this.isAlreadyDark()) {{
                            this.styleEl.textContent = '';
                            document.documentElement.removeAttribute('data-rustium-dark');
                            return;
                        }}

                        document.documentElement.setAttribute('data-rustium-dark', 'true');
                        const curB = (this.config.brightness != null ? this.config.brightness / 100.0 : {b});
                        const curC = (this.config.contrast != null ? this.config.contrast / 100.0 : {c});

                        this.styleEl.textContent = 'html[data-rustium-dark="true"] {{ ' +
                            'filter: invert(90%) hue-rotate(180deg) brightness(' + curB + ') contrast(' + curC + ') !important; ' +
                            'background-color: #181a1b !important; }} ' +
                            'html[data-rustium-dark="true"] img, html[data-rustium-dark="true"] video, html[data-rustium-dark="true"] canvas, ' +
                            'html[data-rustium-dark="true"] svg, html[data-rustium-dark="true"] [role="img"], html[data-rustium-dark="true"] iframe {{ ' +
                            'filter: invert(100%) hue-rotate(180deg) !important; }} ' +
                            'html[data-rustium-dark="true"] img img, html[data-rustium-dark="true"] video video {{ filter: none !important; }}';
                    }}
                }}

                window.__rustium_dark_mode__ = new RustiumDarkReader({{
                    enabled: {enabled},
                    brightness: {brightness},
                    contrast: {contrast}
                }});
            }})();"#
        )
    }
}
