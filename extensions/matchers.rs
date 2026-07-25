//! URL / glob matching for extension content scripts, host permissions, and
//! `web_accessible_resources`.

const ALL_URLS_SCHEMES: &[&str] = &["http", "https", "file", "ftp", "ws", "wss", "data"];

/// True if `url` is matched by an extension match pattern such as
/// `*://*.example.com/*` or `<all_urls>`.
pub fn url_matches_pattern(pattern: &str, url: &str) -> bool {
    if pattern == "<all_urls>" {
        return url::Url::parse(url)
            .map(|u| ALL_URLS_SCHEMES.contains(&u.scheme()))
            .unwrap_or(false);
    }

    let Ok(parsed) = url::Url::parse(url) else {
        return false;
    };

    let (scheme, rest) = if let Some(rest) = pattern.strip_prefix("*://") {
        ("*", rest)
    } else if let Some((scheme, rest)) = pattern.split_once("://") {
        (scheme, rest)
    } else {
        return false;
    };

    if scheme != "*" && scheme != parsed.scheme() {
        return false;
    }

    let (host_part, path) = rest.split_once('/').unwrap_or((rest, ""));
    let path_part = if path.is_empty() { "/*" } else { path };
    let path_part = format!("/{}", path_part);
    if !host_matches(host_part, parsed.host_str().unwrap_or("")) {
        return false;
    }

    glob_match(&path_part, parsed.path())
}

/// Host matching for patterns like `*.example.com`, `*`, or `example.com`.
pub fn host_matches(pattern: &str, host: &str) -> bool {
    if pattern.is_empty() {
        return host.is_empty();
    }
    if let Some(suffix) = pattern.strip_prefix("*.") {
        host == suffix || host.ends_with(&format!(".{}", suffix))
    } else if pattern == "*" {
        true
    } else {
        host == pattern
    }
}

/// Simple glob with `*` matching any sequence of characters and `?` matching
/// any single character.
pub fn glob_match(pattern: &str, target: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let target: Vec<char> = target.chars().collect();
    let mut p = 0usize;
    let mut t = 0usize;
    let mut star_p = None;
    let mut star_t = None;

    while t < target.len() {
        if p < pattern.len() && (pattern[p] == '?' || pattern[p] == target[t]) {
            p += 1;
            t += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star_p = Some(p);
            star_t = Some(t);
            p += 1;
        } else if let Some(sp) = star_p {
            p = sp + 1;
            star_t = Some(star_t.unwrap_or(t) + 1);
            t = star_t.unwrap();
        } else {
            return false;
        }
    }

    while p < pattern.len() && pattern[p] == '*' {
        p += 1;
    }
    p == pattern.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_script_patterns() {
        assert!(url_matches_pattern("*://*.example.com/*", "https://foo.example.com/bar"));
        assert!(!url_matches_pattern("*://*.example.com/*", "https://other.org/"));
        assert!(url_matches_pattern("*://*/foo/*", "https://any.tld/foo/bar"));
        assert!(url_matches_pattern("https://example.com/path/*", "https://example.com/path/to/file"));
        assert!(!url_matches_pattern("https://example.com/path/*", "https://example.com/other"));
        assert!(url_matches_pattern("<all_urls>", "https://anywhere.test/"));
        assert!(!url_matches_pattern("<all_urls>", "chrome://settings/"));
        assert!(url_matches_pattern("file:///foo/*", "file:///foo/bar.txt"));
    }

    #[test]
    fn glob_basic() {
        assert!(glob_match("*", "/anything"));
        assert!(glob_match("/foo/*", "/foo/bar"));
        assert!(glob_match("/foo/*/baz", "/foo/bar/baz"));
        assert!(!glob_match("/foo/*/baz", "/foo/bar/qux"));
        assert!(glob_match("/foo/*.js", "/foo/bar.js"));
        assert!(!glob_match("/foo/*.js", "/foo/bar.css"));
    }
}
