//! Navigation history and controller

/// A single navigation entry
#[derive(Debug, Clone)]
pub struct NavigationEntry {
    pub url: String,
    pub title: Option<String>,
    pub timestamp: std::time::Instant,
}

/// Navigation history and control for a tab
pub struct NavigationController {
    entries: Vec<NavigationEntry>,
    current_index: usize,
}

impl NavigationController {
    /// Create a new navigation controller
    pub fn new(initial_url: String) -> Self {
        NavigationController {
            entries: vec![NavigationEntry {
                url: initial_url,
                title: None,
                timestamp: std::time::Instant::now(),
            }],
            current_index: 0,
        }
    }

    /// Push a new navigation entry
    pub fn push(&mut self, url: String) {
        // Remove forward history
        self.entries.truncate(self.current_index + 1);

        // Add new entry
        self.entries.push(NavigationEntry {
            url,
            title: None,
            timestamp: std::time::Instant::now(),
        });
        self.current_index = self.entries.len() - 1;
    }

    /// Go back in history
    pub fn go_back(&mut self) -> Option<String> {
        if self.can_go_back() {
            self.current_index -= 1;
            Some(self.entries[self.current_index].url.clone())
        } else {
            None
        }
    }

    /// Go forward in history
    pub fn go_forward(&mut self) -> Option<String> {
        if self.can_go_forward() {
            self.current_index += 1;
            Some(self.entries[self.current_index].url.clone())
        } else {
            None
        }
    }

    /// Can go back?
    pub fn can_go_back(&self) -> bool {
        self.current_index > 0
    }

    /// Can go forward?
    pub fn can_go_forward(&self) -> bool {
        self.current_index < self.entries.len() - 1
    }

    /// Get current entry
    pub fn current(&self) -> Option<&NavigationEntry> {
        self.entries.get(self.current_index)
    }

    /// Update title of current entry
    pub fn set_current_title(&mut self, title: String) {
        if let Some(entry) = self.entries.get_mut(self.current_index) {
            entry.title = Some(title);
        }
    }

    /// Get all entries
    pub fn entries(&self) -> &[NavigationEntry] {
        &self.entries
    }

    /// Get current index
    pub fn current_index(&self) -> usize {
        self.current_index
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_navigation_controller_new() {
        let initial_url = "https://example.com".to_string();
        let controller = NavigationController::new(initial_url.clone());

        assert_eq!(controller.entries().len(), 1);
        assert_eq!(controller.current_index(), 0);

        let entry = controller.current().expect("Expected a current entry");
        assert_eq!(entry.url, initial_url);
        assert_eq!(entry.title, None);
    }

    #[test]
    fn test_push_adds_entry() {
        let mut nav = NavigationController::new("https://example.com".to_string());
        assert_eq!(nav.entries().len(), 1);
        assert_eq!(nav.current_index(), 0);

        nav.push("https://example.org".to_string());
        assert_eq!(nav.entries().len(), 2);
        assert_eq!(nav.current_index(), 1);
        assert_eq!(nav.current().unwrap().url, "https://example.org");
        assert!(nav.can_go_back());
        assert!(!nav.can_go_forward());
    }

    #[test]
    fn test_push_truncates_forward_history() {
        let mut nav = NavigationController::new("https://example.com".to_string());
        nav.push("https://example.org".to_string());
        nav.push("https://example.net".to_string());

        assert_eq!(nav.entries().len(), 3);
        assert_eq!(nav.current_index(), 2);

        nav.go_back();
        assert_eq!(nav.current_index(), 1);
        assert_eq!(nav.current().unwrap().url, "https://example.org");

        nav.push("https://new.example.com".to_string());

        assert_eq!(nav.entries().len(), 3);
        assert_eq!(nav.current_index(), 2);
        assert_eq!(nav.current().unwrap().url, "https://new.example.com");

        assert!(!nav.can_go_forward());
    }

    #[test]
    fn test_go_back() {
        let mut nav = NavigationController::new("https://example.com".to_string());

        // Initial state: can't go back
        assert!(!nav.can_go_back());
        assert_eq!(nav.go_back(), None);
        assert_eq!(nav.current_index(), 0);

        // Add some history
        nav.push("https://example.org".to_string());
        nav.push("https://example.net".to_string());

        assert_eq!(nav.entries().len(), 3);
        assert_eq!(nav.current_index(), 2);
        assert!(nav.can_go_back());

        // Go back once
        assert_eq!(nav.go_back(), Some("https://example.org".to_string()));
        assert_eq!(nav.current_index(), 1);
        assert_eq!(nav.current().unwrap().url, "https://example.org");

        // Go back again to the beginning
        assert_eq!(nav.go_back(), Some("https://example.com".to_string()));
        assert_eq!(nav.current_index(), 0);
        assert_eq!(nav.current().unwrap().url, "https://example.com");

        // Can't go back any further
        assert!(!nav.can_go_back());
        assert_eq!(nav.go_back(), None);
        assert_eq!(nav.current_index(), 0);
    }
}
