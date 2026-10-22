use base64::{
    engine::general_purpose::STANDARD, engine::general_purpose::URL_SAFE_NO_PAD, Engine as _,
};
use percent_encoding::percent_decode_str;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use thiserror::Error;
use url::Url;
use uuid::Uuid;

#[derive(Error, Debug, Clone, PartialEq)]
pub enum ProfileError {
    #[error("Invalid URI format: {0}")]
    InvalidUri(String),
    #[error("Unsupported protocol scheme: {0}")]
    UnsupportedScheme(String),
    #[error("Invalid UUID: {0}")]
    InvalidUuid(String),
    #[error("Invalid host or port: {0}")]
    InvalidHostOrPort(String),
    #[error("Invalid Shadowsocks payload: {0}")]
    InvalidShadowsocksPayload(String),
    #[error("Missing required parameter: {0}")]
    MissingParameter(String),
    #[error("Profile not found: {0}")]
    NotFound(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VlessConfig {
    pub uuid: String,
    pub flow: Option<String>,
    pub network: Option<String>,
    pub tls: bool,
    pub sni: Option<String>,
    pub fingerprint: Option<String>,
    pub reality: bool,
    pub public_key: Option<String>,
    pub short_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShadowsocksConfig {
    pub method: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ProtocolConfig {
    Vless(VlessConfig),
    Shadowsocks(ShadowsocksConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub server: String,
    pub port: u16,
    pub protocol: ProtocolConfig,
    pub latency_ms: Option<u64>,
}

impl Profile {
    pub fn new_vless(name: String, server: String, port: u16, config: VlessConfig) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            server,
            port,
            protocol: ProtocolConfig::Vless(config),
            latency_ms: None,
        }
    }

    pub fn new_shadowsocks(
        name: String,
        server: String,
        port: u16,
        config: ShadowsocksConfig,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            server,
            port,
            protocol: ProtocolConfig::Shadowsocks(config),
            latency_ms: None,
        }
    }
}

pub fn parse_profile_uri(raw_uri: &str) -> Result<Profile, ProfileError> {
    let trimmed = raw_uri.trim();
    if trimmed.is_empty() {
        return Err(ProfileError::InvalidUri("Empty URI".to_string()));
    }

    if trimmed.starts_with("vless://") {
        parse_vless_uri(trimmed)
    } else if trimmed.starts_with("ss://") {
        parse_shadowsocks_uri(trimmed)
    } else {
        let scheme = trimmed.split("://").next().unwrap_or("unknown");
        Err(ProfileError::UnsupportedScheme(scheme.to_string()))
    }
}

fn parse_vless_uri(raw_uri: &str) -> Result<Profile, ProfileError> {
    let url = Url::parse(raw_uri).map_err(|e| ProfileError::InvalidUri(e.to_string()))?;

    let uuid_str = url.username();
    if uuid_str.is_empty() {
        return Err(ProfileError::MissingParameter("UUID is empty".to_string()));
    }
    Uuid::parse_str(uuid_str).map_err(|_| ProfileError::InvalidUuid(uuid_str.to_string()))?;

    let host = url
        .host_str()
        .ok_or_else(|| ProfileError::InvalidHostOrPort("Missing host".to_string()))?
        .to_string();

    let port = url
        .port()
        .ok_or_else(|| ProfileError::InvalidHostOrPort("Missing port".to_string()))?;

    let name = match url.fragment() {
        Some(frag) => percent_decode_str(frag)
            .decode_utf8()
            .unwrap_or_default()
            .to_string(),
        None => format!("VLESS {}:{}", host, port),
    };

    let mut security = String::new();
    let mut sni = None;
    let mut fp = None;
    let mut pbk = None;
    let mut sid = None;
    let mut flow = None;
    let mut network = None;

    for (k, v) in url.query_pairs() {
        match k.as_ref() {
            "security" => security = v.to_string(),
            "sni" | "peer" => sni = Some(v.to_string()),
            "fp" => fp = Some(v.to_string()),
            "pbk" => pbk = Some(v.to_string()),
            "sid" => sid = Some(v.to_string()),
            "flow" if !v.is_empty() => {
                flow = Some(v.to_string());
            }
            "type" => network = Some(v.to_string()),
            _ => {}
        }
    }

    let is_reality = security.eq_ignore_ascii_case("reality");
    let is_tls = is_reality || security.eq_ignore_ascii_case("tls");

    if is_reality && pbk.is_none() {
        return Err(ProfileError::MissingParameter(
            "REALITY requires 'pbk' (public key)".to_string(),
        ));
    }

    let vless_config = VlessConfig {
        uuid: uuid_str.to_string(),
        flow,
        network,
        tls: is_tls,
        sni,
        fingerprint: fp.or_else(|| {
            if is_reality {
                Some("chrome".to_string())
            } else {
                None
            }
        }),
        reality: is_reality,
        public_key: pbk,
        short_id: sid,
    };

    Ok(Profile {
        id: Uuid::new_v4().to_string(),
        name,
        server: host,
        port,
        protocol: ProtocolConfig::Vless(vless_config),
        latency_ms: None,
    })
}

fn decode_base64_flexible(input: &str) -> Result<Vec<u8>, ProfileError> {
    if let Ok(bytes) = STANDARD.decode(input) {
        return Ok(bytes);
    }
    if let Ok(bytes) = URL_SAFE_NO_PAD.decode(input.trim_end_matches('=')) {
        return Ok(bytes);
    }
    let padded = match input.len() % 4 {
        2 => format!("{}==", input),
        3 => format!("{}=", input),
        _ => input.to_string(),
    };
    STANDARD
        .decode(&padded)
        .or_else(|_| URL_SAFE_NO_PAD.decode(padded.trim_end_matches('=')))
        .map_err(|e| ProfileError::InvalidShadowsocksPayload(e.to_string()))
}

fn parse_shadowsocks_userinfo(user_info_raw: &str) -> Result<(String, String), ProfileError> {
    // 1. If user_info_raw contains a colon, it is in plaintext `method:password` format (SIP002 modern/unencoded)
    // where method or password may contain percent-encoded characters.
    if let Some((method_raw, password_raw)) = user_info_raw.split_once(':') {
        let method = percent_decode_str(method_raw)
            .decode_utf8()
            .map_err(|e| ProfileError::InvalidShadowsocksPayload(e.to_string()))?
            .to_string();
        let password = percent_decode_str(password_raw)
            .decode_utf8()
            .map_err(|e| ProfileError::InvalidShadowsocksPayload(e.to_string()))?
            .to_string();
        return Ok((method, password));
    }

    // 2. Try Base64 decoding (standard SIP002 base64(method:password))
    if let Ok(decoded_bytes) = decode_base64_flexible(user_info_raw) {
        if let Ok(decoded_str) = String::from_utf8(decoded_bytes) {
            if let Some((method_raw, password_raw)) = decoded_str.split_once(':') {
                let method = percent_decode_str(method_raw)
                    .decode_utf8()
                    .map_err(|e| ProfileError::InvalidShadowsocksPayload(e.to_string()))?
                    .to_string();
                let password = percent_decode_str(password_raw)
                    .decode_utf8()
                    .map_err(|e| ProfileError::InvalidShadowsocksPayload(e.to_string()))?
                    .to_string();
                return Ok((method, password));
            }
        }
    }

    // 3. Try percent-decoding user_info_raw in case the separating colon was encoded as %3A
    let pct_decoded = percent_decode_str(user_info_raw)
        .decode_utf8()
        .map_err(|e| ProfileError::InvalidShadowsocksPayload(e.to_string()))?;
    if let Some((method_raw, password_raw)) = pct_decoded.split_once(':') {
        return Ok((method_raw.to_string(), password_raw.to_string()));
    }

    Err(ProfileError::InvalidShadowsocksPayload(
        "Missing colon in method:password".to_string(),
    ))
}

fn parse_shadowsocks_uri(raw_uri: &str) -> Result<Profile, ProfileError> {
    let trimmed = raw_uri.trim();
    if !trimmed.starts_with("ss://") {
        return Err(ProfileError::UnsupportedScheme(
            "Not a ss:// URI".to_string(),
        ));
    }

    let without_prefix = &trimmed["ss://".len()..];
    let (body, fragment) = match without_prefix.split_once('#') {
        Some((b, f)) => (b, Some(f)),
        None => (without_prefix, None),
    };

    let name = match fragment {
        Some(frag) => {
            let decoded = percent_decode_str(frag)
                .decode_utf8()
                .unwrap_or_default()
                .to_string();
            if decoded.trim().is_empty() {
                "Shadowsocks Server".to_string()
            } else {
                decoded
            }
        }
        None => "Shadowsocks Server".to_string(),
    };

    if let Some((user_info_raw, host_port)) = body.split_once('@') {
        let (method, password) = parse_shadowsocks_userinfo(user_info_raw)?;

        let host_port_clean = host_port
            .split('?')
            .next()
            .unwrap_or(host_port)
            .trim_end_matches('/');
        let (server_raw, port_str) = host_port_clean.rsplit_once(':').ok_or_else(|| {
            ProfileError::InvalidHostOrPort("Missing port in server:port".to_string())
        })?;

        let server = server_raw.trim_start_matches('[').trim_end_matches(']');
        let port: u16 = port_str
            .parse()
            .map_err(|_| ProfileError::InvalidHostOrPort(format!("Invalid port: {}", port_str)))?;

        return Ok(Profile {
            id: Uuid::new_v4().to_string(),
            name,
            server: server.to_string(),
            port,
            protocol: ProtocolConfig::Shadowsocks(ShadowsocksConfig { method, password }),
            latency_ms: None,
        });
    }

    let decoded_body = decode_base64_flexible(body)?;
    let body_str = String::from_utf8(decoded_body)
        .map_err(|e| ProfileError::InvalidShadowsocksPayload(e.to_string()))?;

    let (user_info, host_port) = body_str.split_once('@').ok_or_else(|| {
        ProfileError::InvalidShadowsocksPayload("Missing @ in decoded ss payload".to_string())
    })?;

    let (method, password) = parse_shadowsocks_userinfo(user_info)?;

    let host_port_clean = host_port
        .split('?')
        .next()
        .unwrap_or(host_port)
        .trim_end_matches('/');
    let (server_raw, port_str) = host_port_clean.rsplit_once(':').ok_or_else(|| {
        ProfileError::InvalidHostOrPort("Missing port in server:port".to_string())
    })?;

    let server = server_raw.trim_start_matches('[').trim_end_matches(']');
    let port: u16 = port_str
        .parse()
        .map_err(|_| ProfileError::InvalidHostOrPort(format!("Invalid port: {}", port_str)))?;

    Ok(Profile {
        id: Uuid::new_v4().to_string(),
        name,
        server: server.to_string(),
        port,
        protocol: ProtocolConfig::Shadowsocks(ShadowsocksConfig { method, password }),
        latency_ms: None,
    })
}

#[derive(Clone, Default)]
pub struct ProfileManager {
    profiles: Arc<RwLock<Vec<Profile>>>,
}

impl ProfileManager {
    pub fn new(initial: Vec<Profile>) -> Self {
        Self {
            profiles: Arc::new(RwLock::new(initial)),
        }
    }

    pub fn list(&self) -> Vec<Profile> {
        self.profiles.read().map(|l| l.clone()).unwrap_or_default()
    }

    pub fn get(&self, id: &str) -> Option<Profile> {
        self.profiles
            .read()
            .ok()?
            .iter()
            .find(|p| p.id == id)
            .cloned()
    }

    pub fn add(&self, profile: Profile) {
        if let Ok(mut list) = self.profiles.write() {
            list.push(profile);
        }
    }

    pub fn import_uri(&self, uri: &str) -> Result<Profile, ProfileError> {
        let profile = parse_profile_uri(uri)?;
        self.add(profile.clone());
        Ok(profile)
    }

    pub fn update(&self, profile: Profile) -> Result<(), ProfileError> {
        let mut list = self
            .profiles
            .write()
            .map_err(|_| ProfileError::NotFound("Lock error".to_string()))?;
        if let Some(existing) = list.iter_mut().find(|p| p.id == profile.id) {
            *existing = profile;
            Ok(())
        } else {
            Err(ProfileError::NotFound(profile.id))
        }
    }

    pub fn delete(&self, id: &str) -> Result<Profile, ProfileError> {
        let mut list = self
            .profiles
            .write()
            .map_err(|_| ProfileError::NotFound("Lock error".to_string()))?;
        if let Some(pos) = list.iter().position(|p| p.id == id) {
            Ok(list.remove(pos))
        } else {
            Err(ProfileError::NotFound(id.to_string()))
        }
    }

    pub fn set_profiles(&self, profiles: Vec<Profile>) {
        if let Ok(mut list) = self.profiles.write() {
            *list = profiles;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_vless_reality() {
        let uri = "vless://b831381d-6324-4d53-ad4f-8cda98b3f811@example.com:443?security=reality&sni=test.com&fp=chrome&pbk=enlGdIoYJfgX5TRxi5iQnDDFG-6TVwrPLtj00OdXvlM&sid=1234&flow=xtls-rprx-vision&type=tcp#MyRealityServer";
        let profile = parse_profile_uri(uri).expect("Should parse VLESS Reality URI");
        assert_eq!(profile.name, "MyRealityServer");
        assert_eq!(profile.server, "example.com");
        assert_eq!(profile.port, 443);
        match profile.protocol {
            ProtocolConfig::Vless(cfg) => {
                assert_eq!(cfg.uuid, "b831381d-6324-4d53-ad4f-8cda98b3f811");
                assert!(cfg.reality);
                assert!(cfg.tls);
                assert_eq!(cfg.sni, Some("test.com".to_string()));
                assert_eq!(cfg.fingerprint, Some("chrome".to_string()));
                assert_eq!(
                    cfg.public_key,
                    Some("enlGdIoYJfgX5TRxi5iQnDDFG-6TVwrPLtj00OdXvlM".to_string())
                );
                assert_eq!(cfg.flow, Some("xtls-rprx-vision".to_string()));
            }
            _ => panic!("Expected Vless config"),
        }
    }

    #[test]
    fn test_parse_valid_shadowsocks_sip002() {
        let creds = STANDARD.encode("chacha20-ietf-poly1305:mypassword123");
        let uri = format!("ss://{}@1.2.3.4:8388#MyShadowsocks", creds);
        let profile = parse_profile_uri(&uri).expect("Should parse SIP002 Shadowsocks URI");
        assert_eq!(profile.name, "MyShadowsocks");
        assert_eq!(profile.server, "1.2.3.4");
        assert_eq!(profile.port, 8388);
        match profile.protocol {
            ProtocolConfig::Shadowsocks(cfg) => {
                assert_eq!(cfg.method, "chacha20-ietf-poly1305");
                assert_eq!(cfg.password, "mypassword123");
            }
            _ => panic!("Expected Shadowsocks config"),
        }
    }

    #[test]
    fn test_parse_valid_shadowsocks_2022_multi_user() {
        let uri = "ss://2022-blake3-aes-256-gcm:eJLKUbnWBkXSgMfvFhOkm8kwNPl276dlduExy7WRDdY%3D%3Akuud4PF5VKYiaspCvsZ4qNaqZGI4MAQkNM8AaIKtT%2Fg%3D@jools.com.ru:39674#z041uzf1";
        let profile = parse_profile_uri(uri).expect("Should parse Shadowsocks 2022 multi-user URI");
        assert_eq!(profile.name, "z041uzf1");
        assert_eq!(profile.server, "jools.com.ru");
        assert_eq!(profile.port, 39674);
        match profile.protocol {
            ProtocolConfig::Shadowsocks(cfg) => {
                assert_eq!(cfg.method, "2022-blake3-aes-256-gcm");
                assert_eq!(
                    cfg.password,
                    "eJLKUbnWBkXSgMfvFhOkm8kwNPl276dlduExy7WRDdY=:kuud4PF5VKYiaspCvsZ4qNaqZGI4MAQkNM8AaIKtT/g="
                );
            }
            _ => panic!("Expected Shadowsocks config"),
        }
    }

    #[test]
    fn test_parse_valid_shadowsocks_plain_sip002() {
        let uri = "ss://aes-128-gcm:plainpassword@myhost.net:8388#PlainSS";
        let profile = parse_profile_uri(uri).expect("Should parse plain SIP002 Shadowsocks URI");
        assert_eq!(profile.name, "PlainSS");
        assert_eq!(profile.server, "myhost.net");
        assert_eq!(profile.port, 8388);
        match profile.protocol {
            ProtocolConfig::Shadowsocks(cfg) => {
                assert_eq!(cfg.method, "aes-128-gcm");
                assert_eq!(cfg.password, "plainpassword");
            }
            _ => panic!("Expected Shadowsocks config"),
        }
    }

    #[test]
    fn test_parse_valid_shadowsocks_legacy() {
        let payload = STANDARD.encode("aes-256-gcm:pass123@ss.example.org:9000");
        let uri = format!("ss://{}#LegacySS", payload);
        let profile = parse_profile_uri(&uri).expect("Should parse legacy Shadowsocks URI");
        assert_eq!(profile.name, "LegacySS");
        assert_eq!(profile.server, "ss.example.org");
        assert_eq!(profile.port, 9000);
        match profile.protocol {
            ProtocolConfig::Shadowsocks(cfg) => {
                assert_eq!(cfg.method, "aes-256-gcm");
                assert_eq!(cfg.password, "pass123");
            }
            _ => panic!("Expected Shadowsocks config"),
        }
    }

    #[test]
    fn test_reject_invalid_vless_uuid() {
        let uri = "vless://not-a-uuid@example.com:443#Test";
        let res = parse_profile_uri(uri);
        assert!(matches!(res, Err(ProfileError::InvalidUuid(_))));
    }

    #[test]
    fn test_reject_invalid_scheme() {
        let uri = "vmess://eyJhZGQiOiIxLjIuMy40In0=";
        let res = parse_profile_uri(uri);
        assert!(matches!(res, Err(ProfileError::UnsupportedScheme(_))));
    }

    #[test]
    fn test_reject_empty_uri() {
        let res = parse_profile_uri("   ");
        assert!(matches!(res, Err(ProfileError::InvalidUri(_))));
    }

    #[test]
    fn test_profile_manager_crud() {
        let mgr = ProfileManager::default();
        let uri = "vless://b831381d-6324-4d53-ad4f-8cda98b3f811@example.com:443#Server1";
        let p1 = mgr.import_uri(uri).unwrap();
        assert_eq!(mgr.list().len(), 1);

        let mut p1_mod = p1.clone();
        p1_mod.name = "Renamed Server".to_string();
        mgr.update(p1_mod.clone()).unwrap();
        assert_eq!(mgr.get(&p1.id).unwrap().name, "Renamed Server");

        mgr.delete(&p1.id).unwrap();
        assert_eq!(mgr.list().len(), 0);
    }
}
