use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum RuleAction {
    #[default]
    Direct,
    Vpn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainMatchType {
    Exact,
    Suffix,
    Keyword,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoutingRule {
    pub id: String,
    pub pattern: String,
    pub action: RuleAction,
    pub enabled: bool,
    #[serde(default)]
    pub match_type: Option<DomainMatchType>,
}

impl RoutingRule {
    pub fn new(pattern: &str, action: RuleAction) -> Self {
        let (cleaned_pattern, match_type) = determine_pattern_type(pattern);
        Self {
            id: Uuid::new_v4().to_string(),
            pattern: cleaned_pattern,
            action,
            enabled: true,
            match_type: Some(match_type),
        }
    }

    pub fn get_match_type(&self) -> DomainMatchType {
        self.match_type
            .unwrap_or_else(|| determine_pattern_type(&self.pattern).1)
    }

    /// Formats the rule for sing-box configuration
    pub fn to_singbox_rule_target(&self) -> (&'static str, String, &'static str) {
        let match_type = self.get_match_type();
        let target_outbound = match self.action {
            RuleAction::Direct => "direct",
            RuleAction::Vpn => "proxy",
        };

        match match_type {
            DomainMatchType::Exact => ("domain", self.pattern.clone(), target_outbound),
            DomainMatchType::Suffix => ("domain_suffix", self.pattern.clone(), target_outbound),
            DomainMatchType::Keyword => ("domain_keyword", self.pattern.clone(), target_outbound),
        }
    }
}

pub fn determine_pattern_type(pattern: &str) -> (String, DomainMatchType) {
    let trimmed = pattern.trim().to_lowercase();
    if let Some(suffix) = trimmed.strip_prefix("*.") {
        (suffix.to_string(), DomainMatchType::Suffix)
    } else if let Some(suffix) = trimmed.strip_prefix('.') {
        (suffix.to_string(), DomainMatchType::Suffix)
    } else if trimmed.contains('.') {
        (trimmed, DomainMatchType::Suffix)
    } else {
        (trimmed, DomainMatchType::Keyword)
    }
}

#[derive(Clone)]
pub struct RoutingManager {
    rules: Arc<RwLock<Vec<RoutingRule>>>,
    default_action: Arc<RwLock<RuleAction>>,
}

impl Default for RoutingManager {
    fn default() -> Self {
        Self::new(vec![], RuleAction::Vpn)
    }
}

impl RoutingManager {
    pub fn new(rules: Vec<RoutingRule>, default_action: RuleAction) -> Self {
        Self {
            rules: Arc::new(RwLock::new(rules)),
            default_action: Arc::new(RwLock::new(default_action)),
        }
    }

    pub fn get_rules(&self) -> Vec<RoutingRule> {
        self.rules.read().map(|r| r.clone()).unwrap_or_default()
    }

    pub fn get_default_action(&self) -> RuleAction {
        self.default_action
            .read()
            .map(|a| *a)
            .unwrap_or(RuleAction::Vpn)
    }

    pub fn set_default_action(&self, action: RuleAction) {
        if let Ok(mut lock) = self.default_action.write() {
            *lock = action;
        }
    }

    pub fn set_rules(&self, rules: Vec<RoutingRule>) {
        if let Ok(mut list) = self.rules.write() {
            *list = rules;
        }
    }

    pub fn add_rule(&self, pattern: &str, action: RuleAction) -> RoutingRule {
        let rule = RoutingRule::new(pattern, action);
        if let Ok(mut list) = self.rules.write() {
            list.push(rule.clone());
        }
        rule
    }

    pub fn update_rule(&self, rule: RoutingRule) -> bool {
        if let Ok(mut list) = self.rules.write() {
            if let Some(pos) = list.iter().position(|r| r.id == rule.id) {
                list[pos] = rule;
                return true;
            }
        }
        false
    }

    pub fn delete_rule(&self, id: &str) -> bool {
        if let Ok(mut list) = self.rules.write() {
            if let Some(pos) = list.iter().position(|r| r.id == id) {
                list.remove(pos);
                return true;
            }
        }
        false
    }

    pub fn move_rule(&self, from_index: usize, to_index: usize) -> bool {
        if let Ok(mut list) = self.rules.write() {
            if from_index < list.len() && to_index < list.len() {
                let item = list.remove(from_index);
                list.insert(to_index, item);
                return true;
            }
        }
        false
    }

    pub fn export_rules_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.get_rules())
    }

    pub fn import_rules_json(&self, json: &str) -> Result<usize, serde_json::Error> {
        let imported: Vec<RoutingRule> = serde_json::from_str(json)?;
        let count = imported.len();
        if let Ok(mut list) = self.rules.write() {
            for rule in imported {
                if !list.iter().any(|r| r.pattern == rule.pattern) {
                    list.push(rule);
                }
            }
        }
        Ok(count)
    }

    pub fn evaluate_domain(&self, domain: &str) -> RuleAction {
        let d = domain.to_lowercase();
        if let Ok(list) = self.rules.read() {
            for rule in list.iter() {
                if !rule.enabled {
                    continue;
                }
                let matches = match rule.get_match_type() {
                    DomainMatchType::Exact => d == rule.pattern,
                    DomainMatchType::Suffix => {
                        d == rule.pattern || d.ends_with(&format!(".{}", rule.pattern))
                    }
                    DomainMatchType::Keyword => d.contains(&rule.pattern),
                };
                if matches {
                    return rule.action;
                }
            }
        }

        self.get_default_action()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_type_detection() {
        let (p1, t1) = determine_pattern_type("*.example.com");
        assert_eq!(p1, "example.com");
        assert_eq!(t1, DomainMatchType::Suffix);

        let (p2, t2) = determine_pattern_type(".example.org");
        assert_eq!(p2, "example.org");
        assert_eq!(t2, DomainMatchType::Suffix);

        let (p3, t3) = determine_pattern_type("google");
        assert_eq!(p3, "google");
        assert_eq!(t3, DomainMatchType::Keyword);
    }

    #[test]
    fn test_rule_priority_and_order() {
        let mgr = RoutingManager::new(vec![], RuleAction::Vpn);
        mgr.add_rule("vpn.example.com", RuleAction::Vpn);
        mgr.add_rule("*.example.com", RuleAction::Direct);

        assert_eq!(mgr.evaluate_domain("vpn.example.com"), RuleAction::Vpn);
        assert_eq!(mgr.evaluate_domain("blog.example.com"), RuleAction::Direct);
        assert_eq!(mgr.evaluate_domain("other.org"), RuleAction::Vpn);
    }

    #[test]
    fn test_crud_and_reorder() {
        let mgr = RoutingManager::default();
        let r1 = mgr.add_rule("site1.com", RuleAction::Direct);
        let r2 = mgr.add_rule("site2.com", RuleAction::Vpn);

        assert_eq!(mgr.get_rules().len(), 2);
        assert_eq!(mgr.get_rules()[0].id, r1.id);

        mgr.move_rule(1, 0);
        assert_eq!(mgr.get_rules()[0].id, r2.id);

        mgr.delete_rule(&r1.id);
        assert_eq!(mgr.get_rules().len(), 1);
        assert_eq!(mgr.get_rules()[0].id, r2.id);
    }

    #[test]
    fn test_export_import_rules() {
        let mgr1 = RoutingManager::default();
        mgr1.add_rule("export.test", RuleAction::Direct);
        let exported = mgr1.export_rules_json().unwrap();

        let mgr2 = RoutingManager::default();
        let count = mgr2.import_rules_json(&exported).unwrap();
        assert_eq!(count, 1);
        assert_eq!(mgr2.get_rules()[0].pattern, "export.test");
    }
}
