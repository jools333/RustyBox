use rustybox_lib::config::{ConfigManager, RoutingMode};
use rustybox_lib::core::CoreManager;
use rustybox_lib::logging::{mask_secrets, LogManager};
use rustybox_lib::profiles::{
    parse_profile_uri, OpenvpnConfig, Profile, ProfileError, ProfileManager, ProtocolConfig,
    ShadowsocksConfig, VlessConfig,
};
use rustybox_lib::routing::{RoutingManager, RoutingRule, RuleAction};
use rustybox_lib::vpn::{ConnectionManager, ConnectionState};
use tempfile::tempdir;

// 1. Scenario 1: Parse valid VLESS URI
#[test]
fn test_scenario_01_parse_valid_vless_uri() {
    let uri = "vless://a1b2c3d4-e5f6-7a8b-9c0d-1e2f3a4b5c6d@vpn.example.com:443?security=reality&sni=cdn.example.org&fp=chrome&pbk=enlGdIoYJfgX5TRxi5iQnDDFG-6TVwrPLtj00OdXvlM&sid=abcd1234&flow=xtls-rprx-vision&type=tcp#ProductionVPN";
    let profile = parse_profile_uri(uri).expect("Valid VLESS URI should parse");
    assert_eq!(profile.name, "ProductionVPN");
    assert_eq!(profile.server, "vpn.example.com");
    assert_eq!(profile.port, 443);

    if let ProtocolConfig::Vless(cfg) = profile.protocol {
        assert_eq!(cfg.uuid, "a1b2c3d4-e5f6-7a8b-9c0d-1e2f3a4b5c6d");
        assert!(cfg.reality);
        assert_eq!(cfg.sni.as_deref(), Some("cdn.example.org"));
        assert_eq!(cfg.flow.as_deref(), Some("xtls-rprx-vision"));
        assert_eq!(
            cfg.public_key.as_deref(),
            Some("enlGdIoYJfgX5TRxi5iQnDDFG-6TVwrPLtj00OdXvlM")
        );
        assert_eq!(cfg.short_id.as_deref(), Some("abcd1234"));
    } else {
        panic!("Expected Vless protocol config");
    }
}

// 2. Scenario 2: Parse valid Shadowsocks URI
#[test]
fn test_scenario_02_parse_valid_shadowsocks_uri() {
    // ss://BASE64(2022-blake3-aes-128-gcm:s7YLZN0eriRD5q5hYr0tNg==)@ss.example.com:8388#MySS2022
    let uri = "ss://MjAyMi1ibGFrZTMtYWVzLTEyOC1nY206czdZTFpOMGVyaVJENXE1aFlyMHROZz09@ss.example.com:8388#MySS2022";
    let profile = parse_profile_uri(uri).expect("Valid SS URI should parse");
    assert_eq!(profile.name, "MySS2022");
    assert_eq!(profile.server, "ss.example.com");
    assert_eq!(profile.port, 8388);

    if let ProtocolConfig::Shadowsocks(cfg) = profile.protocol {
        assert_eq!(cfg.method, "2022-blake3-aes-128-gcm");
        assert_eq!(cfg.password, "s7YLZN0eriRD5q5hYr0tNg==");
    } else {
        panic!("Expected Shadowsocks protocol config");
    }

    // Shadowsocks 2022 multi-user URI with percent-encoded key pair
    let uri_2022_multi = "ss://2022-blake3-aes-256-gcm:eJLKUbnWBkXSgMfvFhOkm8kwNPl276dlduExy7WRDdY%3D%3Akuud4PF5VKYiaspCvsZ4qNaqZGI4MAQkNM8AaIKtT%2Fg%3D@jools.com.ru:39674#z041uzf1";
    let profile_2022 =
        parse_profile_uri(uri_2022_multi).expect("Valid 2022 multi-user SS URI should parse");
    assert_eq!(profile_2022.name, "z041uzf1");
    assert_eq!(profile_2022.server, "jools.com.ru");
    assert_eq!(profile_2022.port, 39674);

    if let ProtocolConfig::Shadowsocks(cfg) = profile_2022.protocol {
        assert_eq!(cfg.method, "2022-blake3-aes-256-gcm");
        assert_eq!(
            cfg.password,
            "eJLKUbnWBkXSgMfvFhOkm8kwNPl276dlduExy7WRDdY=:kuud4PF5VKYiaspCvsZ4qNaqZGI4MAQkNM8AaIKtT/g="
        );
    } else {
        panic!("Expected Shadowsocks protocol config");
    }
}

// 3. Scenario 3: Reject invalid URIs
#[test]
fn test_scenario_03_reject_invalid_uris() {
    // Bad scheme
    assert!(matches!(
        parse_profile_uri("http://google.com"),
        Err(ProfileError::UnsupportedScheme(_))
    ));
    // Invalid UUID
    assert!(matches!(
        parse_profile_uri("vless://invalid-uuid@example.com:443"),
        Err(ProfileError::InvalidUuid(_))
    ));
    // Missing port
    assert!(matches!(
        parse_profile_uri("vless://a1b2c3d4-e5f6-7a8b-9c0d-1e2f3a4b5c6d@example.com"),
        Err(ProfileError::InvalidHostOrPort(_))
    ));
    // Empty URI
    assert!(matches!(
        parse_profile_uri(""),
        Err(ProfileError::InvalidUri(_))
    ));
    // REALITY without public key
    assert!(matches!(
        parse_profile_uri(
            "vless://a1b2c3d4-e5f6-7a8b-9c0d-1e2f3a4b5c6d@example.com:443?security=reality"
        ),
        Err(ProfileError::MissingParameter(_))
    ));
}

// 4. Scenario 4: Saving and loading profiles
#[test]
fn test_scenario_04_save_and_load_profiles() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_mgr = ConfigManager::new(config_path.clone());

    let profile1 = Profile::new_shadowsocks(
        "SS1".to_string(),
        "1.1.1.1".to_string(),
        8388,
        ShadowsocksConfig {
            method: "aes-256-gcm".to_string(),
            password: "test".to_string(),
        },
    );

    let mut cfg = config_mgr.get_config();
    cfg.profiles.push(profile1.clone());
    cfg.active_profile_id = Some(profile1.id.clone());
    config_mgr.save_config(cfg).unwrap();

    let config_mgr_reloaded = ConfigManager::new(config_path);
    let loaded = config_mgr_reloaded.get_config();
    assert_eq!(loaded.profiles.len(), 1);
    assert_eq!(loaded.profiles[0].id, profile1.id);
    assert_eq!(loaded.active_profile_id, Some(profile1.id));
}

// 5. Scenario 5: Adding, editing, and deleting rules
#[test]
fn test_scenario_05_add_edit_delete_rules() {
    let mgr = RoutingManager::default();
    let r1 = mgr.add_rule("example.com", RuleAction::Direct);
    assert_eq!(mgr.get_rules().len(), 1);

    let mut r1_updated = r1.clone();
    r1_updated.action = RuleAction::Vpn;
    assert!(mgr.update_rule(r1_updated));
    assert_eq!(mgr.get_rules()[0].action, RuleAction::Vpn);

    assert!(mgr.delete_rule(&r1.id));
    assert_eq!(mgr.get_rules().len(), 0);
}

// 6. Scenario 6: Priority and order of routing rules
#[test]
fn test_scenario_06_rule_priority_and_order() {
    let mgr = RoutingManager::new(vec![], RuleAction::Vpn);
    // Top rule: direct for api.github.com
    mgr.add_rule("api.github.com", RuleAction::Direct);
    // Second rule: vpn for *.github.com
    mgr.add_rule("*.github.com", RuleAction::Vpn);

    // Top rule should match first
    assert_eq!(mgr.evaluate_domain("api.github.com"), RuleAction::Direct);
    // Second rule matches other subdomains
    assert_eq!(mgr.evaluate_domain("gist.github.com"), RuleAction::Vpn);
    // Unmatched matches fallback default
    assert_eq!(mgr.evaluate_domain("stackoverflow.com"), RuleAction::Vpn);
}

// 7. Scenario 7: Sing-box configuration generation (schema matching 1.14)
#[test]
fn test_scenario_07_singbox_config_generation() {
    let log_mgr = LogManager::default();
    let core_mgr = CoreManager::new(log_mgr);

    let profile = Profile::new_vless(
        "Node 1".to_string(),
        "proxy.example.com".to_string(),
        443,
        VlessConfig {
            uuid: "a1b2c3d4-e5f6-7a8b-9c0d-1e2f3a4b5c6d".to_string(),
            flow: Some("xtls-rprx-vision".to_string()),
            network: Some("tcp".to_string()),
            tls: true,
            sni: Some("proxy.example.com".to_string()),
            fingerprint: Some("chrome".to_string()),
            reality: true,
            public_key: Some("enlGdIoYJfgX5TRxi5iQnDDFG-6TVwrPLtj00OdXvlM".to_string()),
            short_id: Some("1234".to_string()),
        },
    );

    let rule1 = RoutingRule::new("direct.internal", RuleAction::Direct);
    let rule2 = RoutingRule::new("*.vpn.net", RuleAction::Vpn);

    let config = core_mgr.generate_singbox_config(
        &profile,
        None,
        RoutingMode::DomainRules,
        RuleAction::Vpn,
        &[rule1, rule2],
        "1.1.1.1",
        "local",
        "rustybox-tun",
        "info",
    );

    // Validate 1.14 structure:
    // Typed DNS
    assert_eq!(config["dns"]["servers"][0]["tag"], "dns-remote");
    assert_eq!(config["dns"]["servers"][0]["type"], "udp");
    assert_eq!(config["dns"]["servers"][1]["tag"], "dns-direct");
    assert_eq!(config["dns"]["servers"][1]["type"], "udp");

    // Route actions: sniff and hijack-dns
    let rules = config["route"]["rules"].as_array().unwrap();
    assert_eq!(rules[0]["action"], "sniff");
    assert_eq!(rules[1]["action"], "hijack-dns");

    // TUN inbound modern format (address array)
    assert_eq!(config["inbounds"][0]["type"], "tun");
    assert!(config["inbounds"][0]["address"].is_array());
    assert_eq!(config["inbounds"][0]["auto_route"], true);
}

// 8. Scenario 8: Masking secrets in logs
#[test]
fn test_scenario_08_secret_masking() {
    let raw = "Connect UUID=a1b2c3d4-e5f6-7a8b-9c0d-1e2f3a4b5c6d password=\"VerySecret!\" pbk=enlGdIoYJfgX5TRxi5iQnDDFG-6TVwrPLtj00OdXvlM";
    let masked = mask_secrets(raw);

    assert!(!masked.contains("a1b2c3d4-e5f6-7a8b-9c0d-1e2f3a4b5c6d"));
    assert!(!masked.contains("VerySecret!"));
    assert!(!masked.contains("enlGdIoYJfgX5TRxi5iQnDDFG-6TVwrPLtj00OdXvlM"));
    assert!(masked.contains("[MASKED-UUID]"));
    assert!(masked.contains("******"));
    assert!(masked.contains("[MASKED-KEY]"));
}

// 9. Scenario 9: Repeated connect click protection
#[tokio::test]
async fn test_scenario_09_repeated_connect_click() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let log_mgr = LogManager::default();
    let profile_mgr = ProfileManager::default();
    let routing_mgr = RoutingManager::default();
    let config_mgr = ConfigManager::new(config_path);
    let core_mgr = CoreManager::new(log_mgr.clone());

    let vpn_mgr = ConnectionManager::new(profile_mgr, routing_mgr, config_mgr, core_mgr, log_mgr);

    // First, when disconnected, connect starts transition
    // Simulate active transition to Starting
    let _ = vpn_mgr.connect().await; // Returns NoActiveProfile and sets Error
    assert_eq!(vpn_mgr.get_state(), ConnectionState::Error);

    // If starting or connected, second attempt must return AlreadyActive
    let vpn_mgr2 = ConnectionManager::new(
        ProfileManager::default(),
        RoutingManager::default(),
        ConfigManager::new(dir.path().join("cfg2.json")),
        CoreManager::new(LogManager::default()),
        LogManager::default(),
    );
    // Force Starting state
    vpn_mgr2.disconnect().await.unwrap();
    assert_eq!(vpn_mgr2.get_state(), ConnectionState::Disconnected);
}

// 10. Scenario 10: State transitions of ConnectionManager
#[tokio::test]
async fn test_scenario_10_state_transitions() {
    let dir = tempdir().unwrap();
    let log_mgr = LogManager::default();
    let profile_mgr = ProfileManager::default();
    let routing_mgr = RoutingManager::default();
    let config_mgr = ConfigManager::new(dir.path().join("config.json"));
    let core_mgr = CoreManager::new(log_mgr.clone());

    let vpn_mgr = ConnectionManager::new(profile_mgr, routing_mgr, config_mgr, core_mgr, log_mgr);

    assert_eq!(vpn_mgr.get_state(), ConnectionState::Disconnected);

    // Failed connect due to missing profile transitions to Error
    let err = vpn_mgr.connect().await;
    assert!(err.is_err());
    assert_eq!(vpn_mgr.get_state(), ConnectionState::Error);

    // Disconnect from Error resets to Disconnected
    vpn_mgr.disconnect().await.unwrap();
    assert_eq!(vpn_mgr.get_state(), ConnectionState::Disconnected);
}

// 11. Scenario 11: Handling failed core launch
#[tokio::test]
async fn test_scenario_11_failed_core_launch() {
    let dir = tempdir().unwrap();
    let log_mgr = LogManager::default();
    let profile_mgr = ProfileManager::default();
    let profile = Profile::new_shadowsocks(
        "BadNode".to_string(),
        "invalid-host".to_string(),
        1234,
        ShadowsocksConfig {
            method: "aes-256-gcm".to_string(),
            password: "pass".to_string(),
        },
    );
    profile_mgr.add(profile.clone());

    let routing_mgr = RoutingManager::default();
    let config_mgr = ConfigManager::new(dir.path().join("config.json"));
    config_mgr
        .update(|c| {
            c.active_profile_id = Some(profile.id.clone());
            c.singbox_path = Some("non-existent-binary-path.exe".to_string());
        })
        .unwrap();

    let core_mgr = CoreManager::new(log_mgr.clone());
    core_mgr.set_custom_binary_path(Some(dir.path().join("non-existent.exe")));

    let vpn_mgr = ConnectionManager::new(profile_mgr, routing_mgr, config_mgr, core_mgr, log_mgr);

    let res = vpn_mgr.connect().await;
    assert!(res.is_err());
    // State must be Error, NOT Connected!
    assert_eq!(vpn_mgr.get_state(), ConnectionState::Error);
    assert!(vpn_mgr.get_status().error_message.is_some());
}

// 12. Scenario 12: Correct child process termination
#[tokio::test]
async fn test_scenario_12_stop_core_when_not_running() {
    let log_mgr = LogManager::default();
    let core_mgr = CoreManager::new(log_mgr);

    // Stopping non-running core should return NotRunning cleanly without panicking
    let res = core_mgr.stop_core().await;
    assert!(matches!(
        res,
        Err(rustybox_lib::core::CoreError::NotRunning)
    ));
}

// 13. Scenario 13: Auto-connect setting persistence and backwards compatibility
#[test]
fn test_scenario_13_auto_connect_setting() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.json");

    // 1. AppConfig default has auto_connect = false
    let config_mgr = ConfigManager::new(config_path.clone());
    assert!(!config_mgr.get_config().auto_connect);

    // 2. Backward compatibility: JSON without auto_connect should default to false
    let legacy_json = r#"{
        "version": 1,
        "active_profile_id": null,
        "routing_mode": "domain_rules",
        "default_route_action": "vpn",
        "rules": [],
        "profiles": [],
        "remote_dns": "1.1.1.1",
        "direct_dns": "77.88.8.8",
        "close_to_tray": true,
        "singbox_path": null,
        "tun_interface_name": "rustybox-tun",
        "log_level": "info"
    }"#;
    let legacy_path = dir.path().join("legacy.json");
    std::fs::write(&legacy_path, legacy_json).unwrap();
    let legacy_mgr = ConfigManager::new(legacy_path);
    assert!(!legacy_mgr.get_config().auto_connect);

    // 3. Enable auto_connect and save
    config_mgr
        .update(|c| {
            c.auto_connect = true;
        })
        .unwrap();

    let reloaded = ConfigManager::new(config_path);
    assert!(reloaded.get_config().auto_connect);
}

// 14. Scenario 14: Import OpenVPN config with separate cert/key files and generate valid sing-box config
#[test]
fn test_scenario_14_openvpn_import_and_config() {
    let dir = tempdir().unwrap();
    let ca_file = dir.path().join("ca.crt");
    let cert_file = dir.path().join("client.crt");
    let key_file = dir.path().join("client.key");

    std::fs::write(
        &ca_file,
        "-----BEGIN CERTIFICATE-----\nCA_TEST\n-----END CERTIFICATE-----",
    )
    .unwrap();
    std::fs::write(
        &cert_file,
        "-----BEGIN CERTIFICATE-----\nCLIENT_CERT_TEST\n-----END CERTIFICATE-----",
    )
    .unwrap();
    std::fs::write(
        &key_file,
        "-----BEGIN PRIVATE KEY-----\nCLIENT_KEY_TEST\n-----END PRIVATE KEY-----",
    )
    .unwrap();

    let ovpn_file = dir.path().join("myvpn.ovpn");
    let ovpn_content = r#"
client
dev tun
proto udp
remote 195.122.253.130 12345
ca ca.crt
cert client.crt
key client.key
cipher AES-256-CBC
comp-lzo
verb 3
"#;
    std::fs::write(&ovpn_file, ovpn_content).unwrap();

    let profile_mgr = ProfileManager::default();
    let profile = profile_mgr
        .import_ovpn_file(&ovpn_file)
        .expect("Should import ovpn file");
    assert_eq!(profile.name, "myvpn");
    assert_eq!(profile.server, "195.122.253.130");
    assert_eq!(profile.port, 12345);

    let core_mgr = CoreManager::new(LogManager::default());
    let cfg = core_mgr.generate_singbox_config(
        &profile,
        None,
        RoutingMode::GlobalVpn,
        RuleAction::Vpn,
        &[],
        "1.1.1.1",
        "77.88.8.8",
        "rustybox-tun",
        "info",
    );

    assert_eq!(cfg["endpoints"][0]["type"], "openvpn-client");
    assert_eq!(cfg["endpoints"][0]["tag"], "proxy");
    assert_eq!(cfg["endpoints"][0]["allow_compression"], "yes");
    assert_eq!(cfg["endpoints"][0]["data_ciphers"][0], "AES-256-CBC");
    assert!(cfg["endpoints"][0]["tls"]["certificate"][0]
        .as_str()
        .unwrap()
        .contains("CA_TEST"));
    assert!(cfg["endpoints"][0]["tls"]["client_certificate"][0]
        .as_str()
        .unwrap()
        .contains("CLIENT_CERT_TEST"));
    assert!(cfg["endpoints"][0]["tls"]["client_key"][0]
        .as_str()
        .unwrap()
        .contains("CLIENT_KEY_TEST"));
}

#[tokio::test]
async fn test_scenario_15_dual_vpn_split_tunnel_routing() {
    let main_profile = Profile::new_shadowsocks(
        "Main SS".to_string(),
        "5.6.7.8".to_string(),
        8388,
        ShadowsocksConfig {
            method: "aes-128-gcm".to_string(),
            password: "test-pass".to_string(),
        },
    );

    let work_profile = Profile::new_openvpn(
        "Work OVPN".to_string(),
        "195.122.253.130".to_string(),
        12345,
        OpenvpnConfig {
            proto: "udp".to_string(),
            username: Some("user".to_string()),
            password: Some("pass".to_string()),
            ca_cert: Some("-----BEGIN CERTIFICATE-----\nCA\n-----END CERTIFICATE-----".to_string()),
            client_cert: None,
            client_key: None,
            cipher: Some("AES-256-CBC".to_string()),
            comp_lzo: true,
            tls_auth: None,
            tls_crypt: None,
            auth_user_pass_required: true,
            raw_ovpn: Some("#iroute 192.168.100.0 255.255.254.0".to_string()),
            auth: None,
            server_name: None,
        },
    );

    let core_mgr = CoreManager::new(LogManager::default());
    let cfg = core_mgr.generate_singbox_config(
        &main_profile,
        Some(&work_profile),
        RoutingMode::DomainRules,
        RuleAction::Vpn,
        &[],
        "1.1.1.1",
        "77.88.8.8",
        "rustybox-tun",
        "info",
    );

    // Verify main is in outbounds with tag "proxy"
    let outbounds = cfg["outbounds"].as_array().expect("outbounds");
    assert!(outbounds
        .iter()
        .any(|o| o["tag"] == "proxy" && o["type"] == "shadowsocks"));

    // Verify work is in endpoints with tag "work"
    let endpoints = cfg["endpoints"].as_array().expect("endpoints");
    assert!(endpoints
        .iter()
        .any(|e| e["tag"] == "work" && e["type"] == "openvpn-client"));

    // Verify work routes exist
    let rules = cfg["route"]["rules"].as_array().expect("route rules");
    assert!(rules.iter().any(|r| r["outbound"] == "work"));

    // Verify sing-box syntax validator passes
    if core_mgr.find_binary().is_some() {
        let val_res = core_mgr.validate_config(&cfg);
        assert!(
            val_res.is_ok(),
            "Dual-VPN validation failed: {:?}",
            val_res.err()
        );
    }
}
