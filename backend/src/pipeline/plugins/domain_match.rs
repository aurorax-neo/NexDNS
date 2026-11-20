use std::collections::HashSet;
use std::sync::Arc;
use parking_lot::RwLock;

#[derive(Default, Clone)]
pub struct DomainMatcher {
    pub name: String,
    pub exact: HashSet<String>,
    pub suffix: HashSet<String>,
    pub keywords: Vec<String>,
}

impl DomainMatcher {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            exact: HashSet::new(),
            suffix: HashSet::new(),
            keywords: Vec::new(),
        }
    }

    pub fn load_from_text(&mut self, text: &str) {
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let lower = trimmed.to_lowercase();
            if let Some(rest) = lower.strip_prefix("full:") {
                let clean = rest.trim_end_matches('.').to_string();
                self.exact.insert(clean);
            } else if let Some(rest) = lower.strip_prefix("domain:") {
                let clean = rest.trim_end_matches('.').to_string();
                self.suffix.insert(clean.clone());
                self.exact.insert(clean);
            } else if let Some(rest) = lower.strip_prefix("keyword:") {
                let clean = rest.trim_end_matches('.').to_string();
                self.keywords.push(clean);
            } else {
                let mut d = lower.as_str();
                if let Some(rest) = d.strip_prefix("+.") {
                    d = rest;
                } else if let Some(rest) = d.strip_prefix('.') {
                    d = rest;
                }
                let clean = d.trim_end_matches('.').to_string();
                self.suffix.insert(clean.clone());
                self.exact.insert(clean);
            }
        }
    }

    pub fn match_specificity(&self, qname: &str) -> Option<usize> {
        let clean = qname.trim_end_matches('.').to_lowercase();
        if clean.is_empty() {
            return None;
        }
        if self.exact.contains(&clean) {
            return Some(clean.len() + 10_000);
        }

        let mut sub = clean.as_str();
        if self.suffix.contains(sub) {
            return Some(sub.len());
        }
        while let Some(idx) = sub.find('.') {
            sub = &sub[idx + 1..];
            if self.suffix.contains(sub) {
                return Some(sub.len());
            }
        }

        let mut best_kw: Option<usize> = None;
        for kw in &self.keywords {
            if clean.contains(kw) {
                best_kw = Some(best_kw.map_or(kw.len(), |cur| cur.max(kw.len())));
            }
        }

        best_kw
    }

    #[allow(dead_code)]
    pub fn matches(&self, qname: &str) -> bool {
        self.match_specificity(qname).is_some()
    }
}

pub struct DomainListRegistry {
    matchers: Arc<RwLock<std::collections::HashMap<String, DomainMatcher>>>,
}

impl Default for DomainListRegistry {
    fn default() -> Self {
        Self::new()
    }
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

    pub fn match_specificity(&self, list_name: &str, qname: &str) -> Option<usize> {
        let r = self.matchers.read();
        r.get(list_name).and_then(|matcher| matcher.match_specificity(qname))
    }

    pub fn matches(&self, list_name: &str, qname: &str) -> bool {
        self.match_specificity(list_name, qname).is_some()
    }
}
