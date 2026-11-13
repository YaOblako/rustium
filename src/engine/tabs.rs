use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tab {
    pub id: String,
    pub url: String,
    pub title: String,
    pub favicon: Option<String>,
    pub is_pinned: bool,
    pub is_loading: bool,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    pub history: Vec<String>,
    pub history_idx: usize,
    pub blocked_count: u64,
}

impl Tab {
    pub fn new(id: String, url: String, title: String) -> Self {
        Self {
            id,
            url: url.clone(),
            title,
            favicon: None,
            is_pinned: false,
            is_loading: false,
            can_go_back: false,
            can_go_forward: false,
            history: vec![url],
            history_idx: 0,
            blocked_count: 0,
        }
    }

    pub fn navigate(&mut self, new_url: String, title: Option<String>) {
        if self.history_idx + 1 < self.history.len() {
            self.history.truncate(self.history_idx + 1);
        }
        self.history.push(new_url.clone());
        self.history_idx = self.history.len() - 1;
        self.url = new_url;
        if let Some(t) = title {
            self.title = t;
        }
        self.can_go_back = self.history_idx > 0;
        self.can_go_forward = false;
        self.blocked_count = 0;
    }

    #[allow(dead_code)]
    pub fn go_back(&mut self) -> Option<String> {
        if self.history_idx > 0 {
            self.history_idx -= 1;
            self.url = self.history[self.history_idx].clone();
            self.can_go_back = self.history_idx > 0;
            self.can_go_forward = true;
            Some(self.url.clone())
        } else {
            None
        }
    }

    #[allow(dead_code)]
    pub fn go_forward(&mut self) -> Option<String> {
        if self.history_idx + 1 < self.history.len() {
            self.history_idx += 1;
            self.url = self.history[self.history_idx].clone();
            self.can_go_back = true;
            self.can_go_forward = self.history_idx + 1 < self.history.len();
            Some(self.url.clone())
        } else {
            None
        }
    }
}

pub struct TabManager {
    pub tabs: Vec<Tab>,
    pub active_tab_id: String,
    next_tab_counter: u64,
}

#[allow(dead_code)]
impl TabManager {
    pub fn new() -> Self {
        let initial_tab = Tab::new(
            "tab_1".to_string(),
            "rustium://newtab".to_string(),
            "Новая вкладка".to_string(),
        );

        Self {
            tabs: vec![initial_tab],
            active_tab_id: "tab_1".to_string(),
            next_tab_counter: 2,
        }
    }

    pub fn create_tab(&mut self, url: &str, title: &str) -> String {
        let id = format!("tab_{}", self.next_tab_counter);
        self.next_tab_counter += 1;

        let tab = Tab::new(id.clone(), url.to_string(), title.to_string());
        self.tabs.push(tab);
        self.active_tab_id = id.clone();
        id
    }

    pub fn close_tab(&mut self, id: &str) -> Option<String> {
        if self.tabs.len() <= 1 {
            // Keep at least one tab, navigate to newtab
            if let Some(tab) = self.tabs.first_mut() {
                tab.navigate("rustium://newtab".to_string(), Some("Новая вкладка".to_string()));
            }
            return Some(self.active_tab_id.clone());
        }

        let idx = self.tabs.iter().position(|t| t.id == id)?;
        let was_active = self.active_tab_id == id;
        self.tabs.remove(idx);

        if was_active {
            let new_idx = if idx >= self.tabs.len() {
                self.tabs.len() - 1
            } else {
                idx
            };
            self.active_tab_id = self.tabs[new_idx].id.clone();
            Some(self.active_tab_id.clone())
        } else {
            Some(self.active_tab_id.clone())
        }
    }

    pub fn switch_tab(&mut self, id: &str) -> bool {
        if self.tabs.iter().any(|t| t.id == id) {
            self.active_tab_id = id.to_string();
            true
        } else {
            false
        }
    }

    pub fn pin_tab(&mut self, id: &str, is_pinned: bool) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            tab.is_pinned = is_pinned;
        }
    }

    pub fn get_active_tab(&self) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.id == self.active_tab_id)
    }

    pub fn get_active_tab_mut(&mut self) -> Option<&mut Tab> {
        let active_id = self.active_tab_id.clone();
        self.tabs.iter_mut().find(|t| t.id == active_id)
    }

    pub fn get_tab(&self, id: &str) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.id == id)
    }

    pub fn get_tab_mut(&mut self, id: &str) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|t| t.id == id)
    }
}
