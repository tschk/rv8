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
    /// Cached resource data
    cache: HashMap<String, bytes::Bytes>,
}

impl Preloader {
    pub fn new() -> Self {
        Preloader {
            client: reqwest::Client::new(),
            hints: HashSet::new(),
            completed: HashSet::new(),
            cache: HashMap::new(),
        }
    }

    /// Add a preload hint
    pub fn add_hint(&mut self, hint: PreloadHint) {
        if !self.completed.contains(&hint) {
            self.hints.insert(hint);
        }
    }

    /// Retrieve a resource from the cache
    pub fn get_from_cache(&self, url: &str) -> Option<&bytes::Bytes> {
        self.cache.get(url)
    }

    /// Process pending hints
    pub async fn process(&mut self) {
        let hints: Vec<_> = self.hints.drain().collect();
        let mut futures = Vec::new();

        for hint in hints {
            let client = self.client.clone();
            let hint_clone = hint.clone();
            let future = async move {
                let cache_item = match &hint_clone {
                    PreloadHint::DnsPrefetch(domain) => {
                        // Trigger DNS lookup
                        let _ = tokio::net::lookup_host(format!("{}:443", domain)).await;
                        None
                    }
                    PreloadHint::Preconnect(origin) => {
                        // Establish connection
                        // TODO: Implement connection pool warming
                        let _ = client.head(origin).send().await;
                        None
                    }
                    PreloadHint::Prefetch(url) => {
                        // Fetch resource to cache
                        if let Ok(response) = client.get(url).send().await {
                            if let Ok(bytes) = response.bytes().await {
                                Some((url.to_string(), bytes))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }
                    PreloadHint::Prerender(url) => {
                        // Prerender page in background
                        // TODO: Implement prerendering
                        if let Ok(response) = client.get(url).send().await {
                            let _ = response.bytes().await;
                        }
                        None
                    }
                };
                (hint_clone, cache_item)
            };
            futures.push(future);
        }

        let results = futures::future::join_all(futures).await;

        for (hint, cache_item) in results {
            if let Some((url, bytes)) = cache_item {
                self.cache.insert(url, bytes);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_prefetch_stores_in_cache() {
        let mut preloader = Preloader::new();
        let url = "https://example.com/";

        preloader.add_hint(PreloadHint::Prefetch(url.to_string()));

        // This makes an actual network request.
        preloader.process().await;

        let cached = preloader.get_from_cache(url);
        assert!(cached.is_some(), "Resource should be cached");
        assert!(
            !cached.unwrap().is_empty(),
            "Cached resource should not be empty"
        );
    }
}
