use kdl::KdlDocument;

#[derive(Clone)]
pub struct Config {
    pub port: String,
    pub db_path: String,
    pub static_dir: String,
    pub settings_path: String,
    pub admin_username: String,
    pub admin_password: String,
    pub voter_hash_salt: String,
    pub enable_hsts: bool,
    pub account_secret: String,
    pub secure_cookies: bool,
    pub captcha_enabled: bool,
    pub captcha_difficulty: u32,
    pub captcha_ttl: i64,
}

pub fn load() -> Result<Config, String> {
    let data = std::fs::read_to_string("config.kdl")
        .map_err(|e| format!("reading config.kdl: {e}"))?;
    let doc: KdlDocument = data
        .parse()
        .map_err(|e| format!("parsing config.kdl: {e}"))?;

    let get_str = |section: &str, key: &str| -> Option<String> {
        doc.get(section)?
            .children()?
            .get_arg(key)?
            .as_string()
            .map(str::to_string)
    };
    let get_bool = |section: &str, key: &str| -> Option<bool> {
        doc.get(section)?.children()?.get_arg(key)?.as_bool()
    };
    let get_int = |section: &str, key: &str| -> Option<i64> {
        doc.get(section)?.children()?.get_arg(key)?.as_i64()
    };

    let cfg = Config {
        port: get_str("server", "port").unwrap_or_else(|| "3000".into()),
        db_path: get_str("server", "db-path").unwrap_or_else(|| "data/kppcn.db".into()),
        static_dir: get_str("server", "static-dir").unwrap_or_else(|| "web/static".into()),
        settings_path: get_str("server", "settings-path")
            .unwrap_or_else(|| "data/settings.json".into()),
        admin_username: get_str("admin", "username").unwrap_or_else(|| "admin".into()),
        admin_password: get_str("admin", "password").unwrap_or_default(),
        voter_hash_salt: get_str("security", "voter-hash-salt").unwrap_or_default(),
        enable_hsts: get_bool("security", "enable-hsts").unwrap_or(false),
        account_secret: get_str("account", "secret").unwrap_or_default(),
        secure_cookies: get_bool("account", "secure-cookies").unwrap_or(true),
        captcha_enabled: get_bool("captcha", "enabled").unwrap_or(true),
        captcha_difficulty: get_int("captcha", "base_difficulty").unwrap_or(20) as u32,
        captcha_ttl: get_int("captcha", "ttl").unwrap_or(300),
    };

    validate(&cfg)?;
    Ok(cfg)
}

fn validate(cfg: &Config) -> Result<(), String> {
    let mut missing = Vec::new();
    if cfg.admin_password.is_empty() {
        missing.push("admin.password");
    }
    if cfg.account_secret.is_empty() {
        missing.push("account.secret");
    }
    if cfg.voter_hash_salt.is_empty() {
        missing.push("security.voter-hash-salt");
    }
    if !missing.is_empty() {
        return Err(format!(
            "config.kdl: missing required secrets: {}",
            missing.join(", ")
        ));
    }
    Ok(())
}
