use std::collections::HashSet;
use std::sync::Arc;
use parking_lot::RwLock;

#[derive(Default, Clone)]
pub struct DomainMatcher {
    pub name: String,
    pub exact: HashSet<String>,
    pub suffix: HashSet<String>,
}

impl DomainMatcher {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            exact: HashSet::new(),
            suffix: HashSet::new(),
        }
    }

    pub fn load_from_text(&mut self, text: &str) {
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let domain = trimmed.trim_end_matches('.').to_lowercase();
            if let Some(rest) = domain.strip_prefix("+.") {
                self.suffix.insert(rest.to_string());
            } else if let Some(rest) = domain.strip_prefix('.') {
                self.suffix.insert(rest.to_string());
            } else {
                self.exact.insert(domain.clone());
                self.suffix.insert(domain);
            }
        }
    }

    pub fn matches(&self, qname: &str) -> bool {
        let lower = qname.to_lowercase();
        if self.exact.contains(&lower) {
            return true;
        }

        let mut sub = lower.as_str();
        if self.suffix.contains(sub) {
            return true;
        }
        while let Some(idx) = sub.find('.') {
            sub = &sub[idx + 1..];
            if self.suffix.contains(sub) {
                return true;
            }
        }

        false
    }
}

pub struct DomainListRegistry {
    matchers: Arc<RwLock<std::collections::HashMap<String, DomainMatcher>>>,
}

impl DomainListRegistry {
    pub fn new() -> Self {
        Self {
            matchers: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    pub fn register(&self, matcher: DomainMatcher) {
        let mut w = self.matchers.write();
        w.insert(matcher.name.clone(), matcher);
    }

    pub fn matches(&self, list_name: &str, qname: &str) -> bool {
        let r = self.matchers.read();
        if let Some(matcher) = r.get(list_name) {
            matcher.matches(qname)
        } else {
            false
        }
    }
}
