use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub message: String,
}

#[derive(Clone)]
pub struct LogManager {
    entries: Arc<RwLock<VecDeque<LogEntry>>>,
    max_entries: usize,
}

impl Default for LogManager {
    fn default() -> Self {
        Self::new(1000)
    }
}

impl LogManager {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Arc::new(RwLock::new(VecDeque::with_capacity(max_entries))),
            max_entries,
        }
    }

    pub fn add_log(&self, level: &str, raw_message: &str) {
        let masked = mask_secrets(raw_message);
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("z:\\jvpn\\singbox.log")
            .and_then(|mut f| {
                use std::io::Write;
                writeln!(f, "[{}] [{}] {}", Utc::now().to_rfc3339(), level, masked)
            });
        if let Ok(mut entries) = self.entries.write() {
            if entries.len() >= self.max_entries {
                entries.pop_front();
            }
            entries.push_back(LogEntry {
                timestamp: Utc::now(),
                level: level.to_string(),
                message: masked,
            });
        }
    }

    pub fn get_logs(&self) -> Vec<LogEntry> {
        self.entries
            .read()
            .map(|e| e.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn clear(&self) {
        if let Ok(mut entries) = self.entries.write() {
            entries.clear();
        }
    }

    pub fn export_diagnostic_report(&self, os_info: &str, core_version: &str) -> String {
        let logs = self.get_logs();
        let mut report = String::new();
        report.push_str("=== RustyBox Diagnostic Report ===\n");
        report.push_str(&format!("Generated: {}\n", Utc::now().to_rfc3339()));
        report.push_str(&format!("OS: {}\n", os_info));
        report.push_str(&format!("Core: {}\n", core_version));
        report.push_str("=== Recent Logs ===\n");
        for log in logs {
            report.push_str(&format!(
                "[{}] [{}] {}\n",
                log.timestamp.format("%Y-%m-%d %H:%M:%S"),
                log.level,
                log.message
            ));
        }
        report
    }
}

/// Masks UUIDs, passwords, private/public keys, and URI credentials from text
pub fn mask_secrets(input: &str) -> String {
    let mut out = input.to_string();

    // Mask UUIDs (standard 36-char format)
    if let Ok(re_uuid) =
        Regex::new(r"(?i)\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b")
    {
        out = re_uuid.replace_all(&out, "[MASKED-UUID]").to_string();
    }

    // Mask password fields in JSON or key-value format: "password": "...", password="...", password=...
    if let Ok(re_pwd) = Regex::new(r#"(?i)("?password"?\s*[:=]\s*"?)[^"&\s\r\n]+("?)"#) {
        out = re_pwd.replace_all(&out, "${1}******${2}").to_string();
    }

    // Mask JSON reality public_key fields: "public_key": "..."
    if let Ok(re_key) = Regex::new(r#"(?i)("public_key"\s*:\s*")[^"]+(")"#) {
        out = re_key.replace_all(&out, "${1}[MASKED-KEY]${2}").to_string();
    }

    // Mask JSON reality private_key fields: "private_key": "..."
    if let Ok(re_priv) = Regex::new(r#"(?i)("private_key"\s*:\s*")[^"]+(")"#) {
        out = re_priv
            .replace_all(&out, "${1}[MASKED-PRIVATE-KEY]${2}")
            .to_string();
    }

    // Mask URI reality pbk parameter: pbk=...
    if let Ok(re_pbk) = Regex::new(r#"(?i)(pbk=)[^& \n\r\t]+"#) {
        out = re_pbk.replace_all(&out, "${1}[MASKED-KEY]").to_string();
    }

    // Mask Shadowsocks URI payload: ss://[base64]@...
    if let Ok(re_ss) = Regex::new(r#"(?i)ss://([A-Za-z0-9+/=]+)@"#) {
        out = re_ss
            .replace_all(&out, "ss://[MASKED-CREDENTIALS]@")
            .to_string();
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_uuid() {
        let text = "Connecting with uuid a1b2c3d4-e5f6-7a8b-9c0d-1e2f3a4b5c6d to server";
        let masked = mask_secrets(text);
        assert!(!masked.contains("a1b2c3d4-e5f6-7a8b-9c0d-1e2f3a4b5c6d"));
        assert!(masked.contains("[MASKED-UUID]"));
    }

    #[test]
    fn test_mask_password() {
        let text = r#"{"type": "shadowsocks", "password": "SuperSecretPassword123"}"#;
        let masked = mask_secrets(text);
        assert!(!masked.contains("SuperSecretPassword123"));
        assert!(masked.contains("******"));
    }

    #[test]
    fn test_mask_reality_key() {
        let text = r#"{"reality": {"public_key": "enlGdIoYJfgX5TRxi5iQnDDFG-6TVwrPLtj00OdXvlM"}}"#;
        let masked = mask_secrets(text);
        assert!(!masked.contains("enlGdIoYJfgX5TRxi5iQnDDFG-6TVwrPLtj00OdXvlM"));
        assert!(masked.contains("[MASKED-KEY]"));
    }

    #[test]
    fn test_mask_vless_uri_pbk() {
        let uri = "vless://user@host:443?pbk=enlGdIoYJfgX5TRxi5iQnDDFG&sid=1234#Test";
        let masked = mask_secrets(uri);
        assert!(!masked.contains("enlGdIoYJfgX5TRxi5iQnDDFG"));
        assert!(masked.contains("pbk=[MASKED-KEY]"));
    }
}
