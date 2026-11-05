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
    #[error("Invalid OpenVPN configuration: {0}")]
    InvalidOvpn(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpenvpnConfig {
    pub proto: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub ca_cert: Option<String>,
    pub client_cert: Option<String>,
    pub client_key: Option<String>,
    pub cipher: Option<String>,
    pub comp_lzo: bool,
    pub tls_auth: Option<String>,
    pub tls_crypt: Option<String>,
    pub auth_user_pass_required: bool,
    pub raw_ovpn: Option<String>,
    #[serde(default)]
    pub auth: Option<String>,
    #[serde(default)]
    pub server_name: Option<String>,
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
    Openvpn(OpenvpnConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub server: String,
    pub port: u16,
    pub protocol: ProtocolConfig,
    pub latency_ms: Option<u64>,
    #[serde(default)]
    pub only_network_resources: bool,
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
            only_network_resources: false,
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
            only_network_resources: false,
        }
    }

    pub fn new_openvpn(name: String, server: String, port: u16, config: OpenvpnConfig) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            server,
            port,
            protocol: ProtocolConfig::Openvpn(config),
            latency_ms: None,
            only_network_resources: false,
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
        only_network_resources: false,
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
            only_network_resources: false,
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
        only_network_resources: false,
    })
}

fn tokenize_ovpn_line(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quote: Option<char> = None;

    for ch in line.chars() {
        if let Some(quote_char) = in_quote {
            if ch == quote_char {
                in_quote = None;
            } else {
                current.push(ch);
            }
        } else if ch == '"' || ch == '\'' {
            in_quote = Some(ch);
        } else if ch == '#' || ch == ';' {
            break;
        } else if ch.is_whitespace() {
            if !current.is_empty() {
                tokens.push(current);
                current = String::new();
            }
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

/// Parses an OpenVPN configuration text into a RustyBox Profile.
///
/// If `base_dir` is provided and relative file paths are specified in `ca`, `cert`, `key`,
/// etc., they will be read from `base_dir` and inlined into the resulting profile.
#[allow(clippy::collapsible_match)]
fn load_ovpn_file_ref(
    file_arg: &str,
    base_dir: Option<&std::path::Path>,
    file_desc: &str,
) -> Result<String, ProfileError> {
    if let Some(base) = base_dir {
        let path = if std::path::Path::new(file_arg).is_relative() {
            base.join(file_arg)
        } else {
            std::path::PathBuf::from(file_arg)
        };
        match std::fs::read_to_string(&path) {
            Ok(data) => return Ok(data),
            Err(e) => {
                return Err(ProfileError::InvalidOvpn(format!(
                    "{} file '{}' could not be read: {}",
                    file_desc,
                    path.display(),
                    e
                )));
            }
        }
    }

    let p = std::path::Path::new(file_arg);
    if p.is_file() {
        match std::fs::read_to_string(p) {
            Ok(data) => return Ok(data),
            Err(e) => {
                return Err(ProfileError::InvalidOvpn(format!(
                    "{} file '{}' could not be read: {}",
                    file_desc,
                    p.display(),
                    e
                )));
            }
        }
    }

    Err(ProfileError::InvalidOvpn(format!(
        "External {} file '{}' referenced in .ovpn could not be found. Please import the .ovpn file using the native file picker.",
        file_desc,
        file_arg
    )))
}

pub fn parse_ovpn(
    content: &str,
    base_dir: Option<&std::path::Path>,
    default_name: Option<&str>,
) -> Result<Profile, ProfileError> {
    let mut server: Option<String> = None;
    let mut port: Option<u16> = None;
    let mut proto = "udp".to_string();
    let mut cipher: Option<String> = None;
    let mut comp_lzo = false;
    let mut auth_user_pass_required = false;
    let mut username: Option<String> = None;
    let mut password: Option<String> = None;
    let mut ca_cert: Option<String> = None;
    let mut client_cert: Option<String> = None;
    let mut client_key: Option<String> = None;
    let mut tls_auth: Option<String> = None;
    let mut tls_crypt: Option<String> = None;
    let mut auth: Option<String> = None;
    let mut server_name: Option<String> = None;

    // First pass: extract inline blocks like <ca>...</ca>, <cert>...</cert>, <key>...</key>
    let mut inline_blocks: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    let mut current_block_tag: Option<String> = None;
    let mut current_block_content = String::new();
    let mut non_block_lines = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(ref tag) = current_block_tag {
            let close_tag = format!("</{}>", tag);
            if trimmed.eq_ignore_ascii_case(&close_tag) {
                inline_blocks.insert(tag.clone(), current_block_content.trim().to_string());
                current_block_tag = None;
                current_block_content.clear();
            } else {
                current_block_content.push_str(line);
                current_block_content.push('\n');
            }
            continue;
        }

        if trimmed.starts_with('<') && trimmed.ends_with('>') && !trimmed.starts_with("</") {
            let tag = trimmed[1..trimmed.len() - 1].trim().to_lowercase();
            if matches!(
                tag.as_str(),
                "ca" | "cert" | "key" | "tls-auth" | "tls-crypt" | "auth-user-pass" | "secret"
            ) {
                current_block_tag = Some(tag);
                current_block_content.clear();
                continue;
            }
        }

        non_block_lines.push(line);
    }

    if let Some(ca) = inline_blocks.get("ca") {
        ca_cert = Some(ca.clone());
    }
    if let Some(cert) = inline_blocks.get("cert") {
        client_cert = Some(cert.clone());
    }
    if let Some(key) = inline_blocks.get("key") {
        client_key = Some(key.clone());
    }
    if let Some(ta) = inline_blocks.get("tls-auth") {
        tls_auth = Some(ta.clone());
    }
    if let Some(tc) = inline_blocks.get("tls-crypt") {
        tls_crypt = Some(tc.clone());
    }
    if let Some(creds) = inline_blocks.get("auth-user-pass") {
        auth_user_pass_required = true;
        let mut lines = creds.lines();
        if let Some(u) = lines.next() {
            username = Some(u.trim().to_string());
        }
        if let Some(p) = lines.next() {
            password = Some(p.trim().to_string());
        }
    }

    // Second pass: parse line-by-line directives
    for line in non_block_lines {
        let tokens = tokenize_ovpn_line(line);
        if tokens.is_empty() {
            continue;
        }

        let directive = tokens[0].to_lowercase();
        match directive.as_str() {
            "remote" => {
                if tokens.len() >= 2 {
                    if server.is_none() {
                        server = Some(tokens[1].clone());
                    }
                    if tokens.len() >= 3 && port.is_none() {
                        if let Ok(p) = tokens[2].parse::<u16>() {
                            port = Some(p);
                        }
                    }
                    if tokens.len() >= 4 {
                        let p = tokens[3].to_lowercase();
                        if p.contains("tcp") {
                            proto = "tcp".to_string();
                        } else if p.contains("udp") {
                            proto = "udp".to_string();
                        }
                    }
                }
            }
            "port" | "rport" => {
                if tokens.len() >= 2 && port.is_none() {
                    if let Ok(p) = tokens[1].parse::<u16>() {
                        port = Some(p);
                    }
                }
            }
            "proto" => {
                if tokens.len() >= 2 {
                    let p = tokens[1].to_lowercase();
                    if p.contains("tcp") {
                        proto = "tcp".to_string();
                    } else {
                        proto = "udp".to_string();
                    }
                }
            }
            "dev" => {
                if tokens.len() >= 2 {
                    let dev = tokens[1].to_lowercase();
                    if dev.starts_with("tap") {
                        return Err(ProfileError::InvalidOvpn(
                            "TAP mode is not supported by RustyBox (only TUN mode is supported)"
                                .to_string(),
                        ));
                    }
                }
            }
            "cipher" => {
                if tokens.len() >= 2 && cipher.is_none() {
                    cipher = Some(tokens[1].clone());
                }
            }
            "data-ciphers" => {
                if tokens.len() >= 2 && cipher.is_none() {
                    cipher = Some(tokens[1].clone());
                }
            }
            "comp-lzo" | "compress" => {
                comp_lzo = true;
            }
            "auth-user-pass" => {
                auth_user_pass_required = true;
                if tokens.len() >= 2 && username.is_none() {
                    let file_arg = &tokens[1];
                    if let Some(base) = base_dir {
                        let path = if std::path::Path::new(file_arg).is_relative() {
                            base.join(file_arg)
                        } else {
                            std::path::PathBuf::from(file_arg)
                        };
                        if let Ok(file_content) = std::fs::read_to_string(&path) {
                            let mut lines = file_content.lines();
                            if let Some(u) = lines.next() {
                                username = Some(u.trim().to_string());
                            }
                            if let Some(p) = lines.next() {
                                password = Some(p.trim().to_string());
                            }
                        }
                    }
                }
            }
            "ca" if ca_cert.is_none() && tokens.len() >= 2 => {
                ca_cert = Some(load_ovpn_file_ref(&tokens[1], base_dir, "CA certificate")?);
            }
            "cert" if client_cert.is_none() && tokens.len() >= 2 => {
                client_cert = Some(load_ovpn_file_ref(
                    &tokens[1],
                    base_dir,
                    "Client certificate",
                )?);
            }
            "key" if client_key.is_none() && tokens.len() >= 2 => {
                client_key = Some(load_ovpn_file_ref(
                    &tokens[1],
                    base_dir,
                    "Client private key",
                )?);
            }
            "tls-auth" if tls_auth.is_none() && tokens.len() >= 2 => {
                tls_auth = Some(load_ovpn_file_ref(&tokens[1], base_dir, "TLS Auth key")?);
            }
            "tls-crypt" if tls_crypt.is_none() && tokens.len() >= 2 => {
                tls_crypt = Some(load_ovpn_file_ref(&tokens[1], base_dir, "TLS Crypt key")?);
            }
            "auth" if auth.is_none() && tokens.len() >= 2 => {
                let a = tokens[1].to_uppercase();
                if a != "NONE" {
                    auth = Some(a);
                }
            }
            "verify-x509-name" if server_name.is_none() && tokens.len() >= 2 => {
                let name = tokens[1].trim_matches('"').trim_matches('\'').to_string();
                server_name = Some(name);
            }
            "remote-cert-tls" if server_name.is_none() && tokens.len() >= 2 => {
                let name = tokens[1].trim_matches('"').trim_matches('\'').to_string();
                server_name = Some(name);
            }
            _ => {}
        }
    }

    let server = server.ok_or_else(|| {
        ProfileError::InvalidOvpn("Missing 'remote' server address in ovpn config".to_string())
    })?;
    let port = port.unwrap_or(1194);

    let name = default_name
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("OpenVPN {}:{}", server, port));

    let auth = auth.or_else(|| {
        if let Some(ref c) = cipher {
            if c.to_uppercase().contains("CBC") {
                Some("SHA1".to_string())
            } else {
                None
            }
        } else {
            None
        }
    });

    let config = OpenvpnConfig {
        proto,
        username,
        password,
        ca_cert,
        client_cert,
        client_key,
        cipher,
        comp_lzo,
        tls_auth,
        tls_crypt,
        auth_user_pass_required,
        raw_ovpn: Some(content.to_string()),
        auth,
        server_name,
    };

    Ok(Profile {
        id: Uuid::new_v4().to_string(),
        name,
        server,
        port,
        protocol: ProtocolConfig::Openvpn(config),
        latency_ms: None,
        only_network_resources: false,
    })
}

/// Extracts any IPv4 routes specified in `route` or `iroute` directives in an OpenVPN config.
pub fn extract_ovpn_routes(raw_ovpn: &str) -> Vec<String> {
    let mut routes = Vec::new();
    for line in raw_ovpn.lines() {
        let clean = line
            .trim()
            .trim_start_matches('#')
            .trim_start_matches(';')
            .trim();
        let tokens: Vec<&str> = clean.split_whitespace().collect();
        if tokens.len() >= 2
            && (tokens[0].eq_ignore_ascii_case("route") || tokens[0].eq_ignore_ascii_case("iroute"))
        {
            let ip_str = tokens[1];
            if let Ok(ip) = ip_str.parse::<std::net::Ipv4Addr>() {
                if tokens.len() >= 3 {
                    if let Ok(mask) = tokens[2].parse::<std::net::Ipv4Addr>() {
                        let prefix = u32::from_be_bytes(mask.octets()).count_ones();
                        let cidr = format!("{}/{}", ip, prefix);
                        if !routes.contains(&cidr) {
                            routes.push(cidr);
                        }
                        continue;
                    }
                }
                let cidr = format!("{}/32", ip);
                if !routes.contains(&cidr) {
                    routes.push(cidr);
                }
            }
        }
    }
    routes
}

pub fn parse_ovpn_file(path: &std::path::Path) -> Result<Profile, ProfileError> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| ProfileError::InvalidOvpn(format!("Failed to read ovpn file: {}", e)))?;
    let base_dir = path.parent();
    let default_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty());
    parse_ovpn(&content, base_dir, default_name)
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

    pub fn import_ovpn_file(&self, path: &std::path::Path) -> Result<Profile, ProfileError> {
        let profile = parse_ovpn_file(path)?;
        self.add(profile.clone());
        Ok(profile)
    }

    pub fn import_ovpn_content(
        &self,
        content: &str,
        base_dir: Option<&std::path::Path>,
        name: Option<&str>,
    ) -> Result<Profile, ProfileError> {
        let profile = parse_ovpn(content, base_dir, name)?;
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

    pub fn set_only_network_resources(
        &self,
        id: &str,
        only_network: bool,
    ) -> Result<Profile, ProfileError> {
        let mut list = self
            .profiles
            .write()
            .map_err(|_| ProfileError::NotFound("Lock error".to_string()))?;
        if let Some(existing) = list.iter_mut().find(|p| p.id == id) {
            existing.only_network_resources = only_network;
            Ok(existing.clone())
        } else {
            Err(ProfileError::NotFound(id.to_string()))
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

    #[test]
    fn test_parse_user_ovpn_with_external_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        let base_path = temp_dir.path();

        let ca_file = base_path.join("ca.crt");
        let cert_file = base_path.join("nosyrev26.crt");
        let key_file = base_path.join("nosyrev26.key");

        std::fs::write(
            &ca_file,
            "-----BEGIN CERTIFICATE-----\nCA_CONTENT\n-----END CERTIFICATE-----",
        )
        .unwrap();
        std::fs::write(
            &cert_file,
            "-----BEGIN CERTIFICATE-----\nCERT_CONTENT\n-----END CERTIFICATE-----",
        )
        .unwrap();
        std::fs::write(
            &key_file,
            "-----BEGIN PRIVATE KEY-----\nKEY_CONTENT\n-----END PRIVATE KEY-----",
        )
        .unwrap();

        let ovpn_content = r#"
client
dev tun
proto udp
# Адрес и порт сервера
remote 195.122.253.130 12345
# Ключи должны лежать в папке с конфигом
ca ca.crt
cert nosyrev26.crt
key nosyrev26.key
# remote-cert-tls server
cipher AES-256-CBC
nobind
comp-lzo
persist-key
persist-tun
verb 3
#iroute 192.168.100.0 255.255.254.0
"#;

        let ovpn_file = base_path.join("client.ovpn");
        std::fs::write(&ovpn_file, ovpn_content).unwrap();

        let profile = parse_ovpn_file(&ovpn_file).expect("Should parse user's OpenVPN file");
        assert_eq!(profile.name, "client");
        assert_eq!(profile.server, "195.122.253.130");
        assert_eq!(profile.port, 12345);

        match profile.protocol {
            ProtocolConfig::Openvpn(cfg) => {
                assert_eq!(cfg.proto, "udp");
                assert_eq!(cfg.cipher, Some("AES-256-CBC".to_string()));
                assert!(cfg.comp_lzo);
                assert!(cfg.ca_cert.unwrap().contains("CA_CONTENT"));
                assert!(cfg.client_cert.unwrap().contains("CERT_CONTENT"));
                assert!(cfg.client_key.unwrap().contains("KEY_CONTENT"));
            }
            _ => panic!("Expected Openvpn config"),
        }
    }

    #[test]
    fn test_parse_ovpn_with_inline_blocks() {
        let ovpn_content = r#"
client
dev tun
proto tcp
remote vpn.example.com 443
cipher AES-128-GCM
auth-user-pass
<ca>
-----BEGIN CERTIFICATE-----
INLINE_CA
-----END CERTIFICATE-----
</ca>
<cert>
-----BEGIN CERTIFICATE-----
INLINE_CERT
-----END CERTIFICATE-----
</cert>
<key>
-----BEGIN PRIVATE KEY-----
INLINE_KEY
-----END PRIVATE KEY-----
</key>
"#;

        let profile = parse_ovpn(ovpn_content, None, Some("Corporate VPN")).unwrap();
        assert_eq!(profile.name, "Corporate VPN");
        assert_eq!(profile.server, "vpn.example.com");
        assert_eq!(profile.port, 443);

        match profile.protocol {
            ProtocolConfig::Openvpn(cfg) => {
                assert_eq!(cfg.proto, "tcp");
                assert_eq!(cfg.cipher, Some("AES-128-GCM".to_string()));
                assert!(cfg.auth_user_pass_required);
                assert!(cfg.ca_cert.unwrap().contains("INLINE_CA"));
                assert!(cfg.client_cert.unwrap().contains("INLINE_CERT"));
                assert!(cfg.client_key.unwrap().contains("INLINE_KEY"));
            }
            _ => panic!("Expected Openvpn config"),
        }
    }

    #[test]
    fn test_reject_tap_mode() {
        let ovpn_content = "dev tap\nremote 1.2.3.4 1194";
        let res = parse_ovpn(ovpn_content, None, None);
        assert!(matches!(res, Err(ProfileError::InvalidOvpn(_))));
    }

    #[test]
    fn test_reject_missing_remote() {
        let ovpn_content = "dev tun\nproto udp";
        let res = parse_ovpn(ovpn_content, None, None);
        assert!(matches!(res, Err(ProfileError::InvalidOvpn(_))));
    }

    #[test]
    fn test_parse_ovpn_auth_and_server_name() {
        let ovpn_content = r#"
client
dev tun
proto udp
remote 195.122.253.130 12345
cipher AES-256-CBC
remote-cert-tls server
"#;
        let profile = parse_ovpn(ovpn_content, None, None).unwrap();
        match profile.protocol {
            ProtocolConfig::Openvpn(cfg) => {
                assert_eq!(cfg.auth, Some("SHA1".to_string()));
                assert_eq!(cfg.server_name, Some("server".to_string()));
            }
            _ => panic!("Expected Openvpn config"),
        }

        let ovpn_custom = r#"
client
dev tun
proto udp
remote vpn.corp.local 1194
cipher AES-128-GCM
auth SHA256
verify-x509-name "my-vpn-server" name
"#;
        let profile2 = parse_ovpn(ovpn_custom, None, None).unwrap();
        match profile2.protocol {
            ProtocolConfig::Openvpn(cfg) => {
                assert_eq!(cfg.auth, Some("SHA256".to_string()));
                assert_eq!(cfg.server_name, Some("my-vpn-server".to_string()));
            }
            _ => panic!("Expected Openvpn config"),
        }
    }
}
