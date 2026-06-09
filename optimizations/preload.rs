//! Resource preloading and prefetching

use std::collections::{HashMap, HashSet};

/// Preload hint types
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PreloadHint {
    /// DNS prefetch for domain
    DnsPrefetch(String),
    /// Preconnect to origin
    Preconnect(String),
    /// Prefetch resource
    Prefetch(String),
    /// Prerender page
    Prerender(String),
}

/// Resource preloader
pub struct Preloader {
    client: reqwest::Client,
    /// Active preload hints
    hints: HashSet<PreloadHint>,
    /// Completed preloads
    completed: HashSet<PreloadHint>,
    /// Resource cache
    pub resource_cache: HashMap<String, bytes::Bytes>,
}

impl Preloader {
    pub fn new() -> Self {
        Preloader {
            client: reqwest::Client::new(),
            hints: HashSet::new(),
            completed: HashSet::new(),
            resource_cache: HashMap::new(),
        }
    }

    /// Add a preload hint
    pub fn add_hint(&mut self, hint: PreloadHint) {
        if !self.completed.contains(&hint) {
            self.hints.insert(hint);
        }
    }

    /// Process pending hints
    pub async fn process(&mut self) {
        let hints: Vec<_> = self.hints.drain().collect();

        for hint in hints {
            match &hint {
                PreloadHint::DnsPrefetch(domain) => {
                    // Trigger DNS lookup
                    let _ = tokio::net::lookup_host(format!("{}:443", domain)).await;
                }
                PreloadHint::Preconnect(origin) => {
                    // Establish connection
                    let _ = self.client.head(origin).send().await;
                }
                PreloadHint::Prefetch(url) => {
                    // Fetch resource to cache
                    if let Ok(response) = self.client.get(url).send().await {
                        if let Ok(bytes) = response.bytes().await {
                            self.resource_cache.insert(url.clone(), bytes);
                        }
                    }
                }
                PreloadHint::Prerender(url) => {
                    // Prerender page in background
                    if let Ok(response) = self.client.get(url).send().await {
                        if let Ok(bytes) = response.bytes().await {
                            self.resource_cache.insert(url.clone(), bytes);
                        }
                    }
                }
            }
            self.completed.insert(hint);
        }
    }
}

impl Default for Preloader {
    fn default() -> Self {
        Self::new()
    }
}
