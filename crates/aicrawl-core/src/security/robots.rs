//! robots.txt parsing and path matching.
//!
//! AICrawl respects robots.txt by default. The parser implements the widely
//! deployed subset (user-agent groups, `Allow`, `Disallow`, `*` and `$`
//! wildcards, longest-match precedence, `Crawl-delay`) and ignores `Sitemap`
//! entries. Anything unrecognised is skipped rather than guessed at.

use serde::{Deserialize, Serialize};

/// A single parsed robots.txt rule.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RobotsRule {
    pub allow: bool,
    pub pattern: String,
}

/// One user-agent group.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RobotsGroup {
    pub agents: Vec<String>,
    pub rules: Vec<RobotsRule>,
    pub crawl_delay_secs: Option<u64>,
}

/// Parsed robots.txt document.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RobotsRules {
    pub groups: Vec<RobotsGroup>,
    pub sitemaps: Vec<String>,
}

/// Parsed robots.txt plus fetch metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RobotsSnapshot {
    pub origin: String,
    pub status_code: u16,
    pub rules: RobotsRules,
    pub fetched_at: String,
}

impl RobotsRules {
    /// Parses robots.txt content. Never fails: malformed lines are ignored.
    pub fn parse(body: &str) -> Self {
        let mut groups: Vec<RobotsGroup> = Vec::new();
        let mut current: Option<RobotsGroup> = None;
        let mut sitemaps = Vec::new();
        let mut last_was_agent = false;

        for raw_line in body.lines() {
            let line = raw_line.split('#').next().unwrap_or("").trim();
            if line.is_empty() || !line.contains(':') {
                continue;
            }
            let (field, value) = line.split_once(':').unwrap();
            let field = field.trim().to_ascii_lowercase();
            let value = value.trim();

            match field.as_str() {
                "user-agent" | "useragent" => {
                    if !last_was_agent {
                        if let Some(group) = current.take() {
                            groups.push(group);
                        }
                        current = Some(RobotsGroup::default());
                    }
                    if let Some(group) = current.as_mut() {
                        group.agents.push(value.to_ascii_lowercase());
                    }
                    last_was_agent = true;
                }
                "allow" | "disallow" => {
                    last_was_agent = false;
                    let Some(group) = current.as_mut() else {
                        continue;
                    };
                    // An empty Disallow means "allow everything" for this group and
                    // is represented by the absence of a rule.
                    if value.is_empty() {
                        continue;
                    }
                    group.rules.push(RobotsRule {
                        allow: field == "allow",
                        pattern: value.to_string(),
                    });
                }
                "crawl-delay" => {
                    last_was_agent = false;
                    if let Some(group) = current.as_mut() {
                        group.crawl_delay_secs = value.parse::<f64>().ok().map(|v| v.ceil() as u64);
                    }
                }
                "sitemap" => {
                    last_was_agent = false;
                    if !value.is_empty() {
                        sitemaps.push(value.to_string());
                    }
                }
                _ => {}
            }
        }
        if let Some(group) = current.take() {
            groups.push(group);
        }

        Self { groups, sitemaps }
    }

    /// Selects the group for `user_agent`, preferring the most specific match.
    pub fn group_for<'a>(&'a self, user_agent: &str) -> Option<&'a RobotsGroup> {
        let ua = user_agent.to_ascii_lowercase();
        let mut best: Option<(&RobotsGroup, usize)> = None;
        for group in &self.groups {
            for agent in &group.agents {
                if agent == "*" || agent.is_empty() {
                    continue;
                }
                if ua.contains(agent) {
                    let specificity = agent.len();
                    if best.map(|(_, len)| specificity > len).unwrap_or(true) {
                        best = Some((group, specificity));
                    }
                }
            }
        }
        if let Some((group, _)) = best {
            return Some(group);
        }
        self.groups
            .iter()
            .find(|group| group.agents.iter().any(|agent| agent == "*"))
    }

    /// Longest matching rule decides; ties resolve to "allow".
    pub fn is_allowed(&self, path: &str) -> bool {
        self.is_allowed_for("*", path)
    }

    /// Evaluates `path` (path + query) for a specific user agent.
    pub fn is_allowed_for(&self, user_agent: &str, path: &str) -> bool {
        let Some(group) = self.group_for(user_agent) else {
            return true;
        };
        let mut best: Option<(&RobotsRule, usize)> = None;
        for rule in &group.rules {
            if let Some(len) = match_pattern(&rule.pattern, path) {
                if best.map(|(_, best_len)| len > best_len).unwrap_or(true)
                    || (best.map(|(_, best_len)| len == best_len).unwrap_or(false) && rule.allow)
                {
                    best = Some((rule, len));
                }
            }
        }
        best.map(|(rule, _)| rule.allow).unwrap_or(true)
    }

    /// Crawl delay for a user agent, if declared.
    pub fn crawl_delay(&self, user_agent: &str) -> Option<u64> {
        self.group_for(user_agent).and_then(|g| g.crawl_delay_secs)
    }
}

/// Returns the match length when `pattern` matches `path`, using `*` and `$`.
fn match_pattern(pattern: &str, path: &str) -> Option<usize> {
    if pattern == "/" {
        return Some(0);
    }
    let anchored = pattern.ends_with('$');
    let core = if anchored {
        pattern.trim_end_matches('$')
    } else {
        pattern
    };
    let segments: Vec<&str> = core.split('*').collect();

    if segments.len() == 1 {
        if anchored {
            return (path == core).then(|| core.len());
        }
        return path.starts_with(core).then(|| core.len());
    }

    let mut rest = path;
    let mut consumed = 0usize;
    for (index, segment) in segments.iter().enumerate() {
        if segment.is_empty() {
            continue;
        }
        if index == 0 {
            if !rest.starts_with(segment) {
                return None;
            }
            rest = &rest[segment.len()..];
            consumed += segment.len();
        } else {
            let found = rest.find(segment)?;
            rest = &rest[found + segment.len()..];
            consumed += found + segment.len();
        }
    }

    if anchored && !rest.is_empty() {
        return None;
    }
    Some(consumed)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
# comment
User-agent: *
Disallow: /private/
Allow: /private/public/
Disallow: /*.pdf$
Crawl-delay: 2

User-agent: AICrawl
Disallow: /bot-only/
"#;

    #[test]
    fn wildcard_group_disallows_private() {
        let rules = RobotsRules::parse(SAMPLE);
        assert!(!rules.is_allowed_for("SomeOtherBot", "/private/page"));
        assert!(rules.is_allowed_for("SomeOtherBot", "/private/public/doc"));
        assert!(rules.is_allowed_for("SomeOtherBot", "/"));
        assert!(!rules.is_allowed_for("SomeOtherBot", "/files/report.pdf"));
        assert!(rules.is_allowed_for("SomeOtherBot", "/files/report.pdf?download=1"));
    }

    #[test]
    fn specific_group_wins() {
        let rules = RobotsRules::parse(SAMPLE);
        assert!(!rules.is_allowed_for("AICrawl/1.0", "/bot-only/x"));
        assert!(rules.is_allowed_for("AICrawl/1.0", "/private/page"));
        assert_eq!(rules.crawl_delay("AICrawl/1.0"), None);
        assert_eq!(rules.crawl_delay("SomeBot"), Some(2));
    }

    #[test]
    fn empty_disallow_allows_everything() {
        let rules = RobotsRules::parse("User-agent: *\nDisallow:\n");
        assert!(rules.is_allowed_for("AnyBot", "/anything"));
    }

    #[test]
    fn longest_match_wins() {
        let rules = RobotsRules::parse("User-agent: *\nDisallow: /a/\nAllow: /a/b/\n");
        assert!(!rules.is_allowed_for("x", "/a/c"));
        assert!(rules.is_allowed_for("x", "/a/b/c"));
    }
}
