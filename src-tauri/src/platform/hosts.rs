use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::net::IpAddr;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostsEntries {
    pub domains: Vec<String>,
    pub ip_cidrs: Vec<String>,
}

pub fn get_system_hosts_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(sys_root) = std::env::var("SystemRoot") {
            let p = PathBuf::from(sys_root).join(r"System32\drivers\etc\hosts");
            if p.exists() {
                return p;
            }
        }
        PathBuf::from(r"C:\Windows\System32\drivers\etc\hosts")
    }
    #[cfg(not(target_os = "windows"))]
    {
        PathBuf::from("/etc/hosts")
    }
}

pub fn parse_hosts_content(content: &str) -> HostsEntries {
    let mut domain_set = HashSet::new();
    let mut ip_set = HashSet::new();
    let mut domains = Vec::new();
    let mut ip_cidrs = Vec::new();

    for line in content.lines() {
        let clean = match line.find('#') {
            Some(idx) => &line[..idx],
            None => line,
        }
        .trim();

        if clean.is_empty() {
            continue;
        }

        let parts: Vec<&str> = clean.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }

        let ip_str = parts[0];
        let ip: IpAddr = match ip_str.parse() {
            Ok(addr) => addr,
            Err(_) => continue,
        };

        match ip {
            IpAddr::V4(v4) => {
                if v4.is_loopback()
                    || v4.octets()[0] == 127
                    || v4.is_unspecified()
                    || v4.is_broadcast()
                {
                    continue;
                }
                let cidr = format!("{}/32", v4);
                if ip_set.insert(cidr.clone()) {
                    ip_cidrs.push(cidr);
                }
            }
            IpAddr::V6(v6) => {
                if v6.is_loopback() || v6.is_unspecified() {
                    continue;
                }
                let cidr = format!("{}/128", v6);
                if ip_set.insert(cidr.clone()) {
                    ip_cidrs.push(cidr);
                }
            }
        }

        for &raw_dom in &parts[1..] {
            let dom = raw_dom.trim().to_lowercase();
            if dom.is_empty()
                || dom == "localhost"
                || dom == "broadcasthost"
                || dom == "localdomain"
            {
                continue;
            }
            if domain_set.insert(dom.clone()) {
                domains.push(dom);
            }
        }
    }

    HostsEntries { domains, ip_cidrs }
}

pub fn get_system_hosts_entries() -> HostsEntries {
    let path = get_system_hosts_path();
    match std::fs::read_to_string(&path) {
        Ok(content) => parse_hosts_content(&content),
        Err(_) => HostsEntries::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hosts_content_filters_loopback_and_extracts_corporate() {
        let sample = r#"
# Sample hosts
127.0.0.1 localhost
127.0.1.11 phpmyadmin
::1 localhost

192.168.102.102 forgejo.k8s-dev.local
192.168.102.102 portal.k8s-dev.local
92.101.253.149 ais.k8s-pre-prod.local # work pre-prod
192.168.100.211 commfort commfort.centr.local
"#;
        let entries = parse_hosts_content(sample);
        assert_eq!(
            entries.ip_cidrs,
            vec![
                "192.168.102.102/32",
                "92.101.253.149/32",
                "192.168.100.211/32"
            ]
        );
        assert_eq!(
            entries.domains,
            vec![
                "forgejo.k8s-dev.local",
                "portal.k8s-dev.local",
                "ais.k8s-pre-prod.local",
                "commfort",
                "commfort.centr.local"
            ]
        );
    }
}
