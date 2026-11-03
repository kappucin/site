use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::Response;
use axum::Extension;

use crate::api::{write_error, write_json};
use crate::mw::{admin_check, ClientIp};
use crate::App;

fn raw_json(data: Vec<u8>) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
        .header(header::CACHE_CONTROL, "private, no-store")
        .body(Body::from(data))
        .unwrap()
}

pub async fn get_settings(State(app): State<Arc<App>>) -> Response {

    let mod_time = std::fs::metadata(&app.cfg.settings_path)
        .ok()
        .and_then(|m| m.modified().ok())
        .unwrap_or(UNIX_EPOCH);

    {
        let cache = app.settings_cache.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((data, t)) = cache.as_ref() {
            if *t == mod_time {
                return raw_json(data.clone());
            }
        }
    }

    let data = match std::fs::read(&app.cfg.settings_path) {
        Ok(d) => d,
        Err(e) => {
            tracing::error!(error = %e, path = %app.cfg.settings_path, "settings: read failed");
            return write_error(StatusCode::INTERNAL_SERVER_ERROR, "Settings not found");
        }
    };
    *app.settings_cache.lock().unwrap_or_else(|e| e.into_inner()) = Some((data.clone(), mod_time));
    raw_json(data)
}

fn scheme_of(link: &str) -> Option<&str> {
    for (i, c) in link.char_indices() {
        match c {
            ':' => return Some(&link[..i]),
            '/' | '?' | '#' => return None,
            _ => {}
        }
    }
    None
}

pub fn is_safe_link(link: &str) -> bool {
    match scheme_of(link) {
        None => true,
        Some(s) => {
            let s = s.to_ascii_lowercase();
            s == "http" || s == "https" || s == "mailto" || s == "tel"
        }
    }
}

pub fn is_safe_img_url(link: &str) -> bool {
    match scheme_of(link) {
        None => true,
        Some(s) => {
            let s = s.to_ascii_lowercase();
            s == "http" || s == "https"
        }
    }
}

fn write_file_atomic(path: &str, data: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let tmp = format!("{}.tmp-{}", path, crate::idgen::new_id());
    {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp)?;
        f.write_all(data)?;
    }
    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

pub async fn admin_update_settings(
    State(app): State<Arc<App>>,
    Extension(ClientIp(ip)): Extension<ClientIp>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if let Err(resp) = admin_check(&app, true, &headers, &ip) {
        return resp;
    }

    #[derive(serde::Deserialize)]
    struct Req<'a> {
        #[serde(borrow)]
        settings: &'a serde_json::value::RawValue,
    }

    let req: Req = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(_) => return write_error(StatusCode::BAD_REQUEST, "invalid json"),
    };

    let obj: serde_json::Value = match serde_json::from_str(req.settings.get()) {
        Ok(v) => v,
        Err(_) => return write_error(StatusCode::BAD_REQUEST, "expected a json object"),
    };
    let map = match obj.as_object() {
        Some(m) => m,
        None => return write_error(StatusCode::BAD_REQUEST, "expected a json object"),
    };

    for key in ["contacts"] {
        if let Some(items) = map.get(key).and_then(|v| v.as_array()) {
            for it in items {
                let link = it.get("link").and_then(|v| v.as_str()).unwrap_or("");
                if !link.is_empty() && !is_safe_link(link) {
                    return write_error(
                        StatusCode::BAD_REQUEST,
                        "unsupported link scheme (allowed: http, https, mailto, tel)",
                    );
                }
            }
        }
    }
    if let Some(emojis) = map.get("emojis").and_then(|v| v.as_array()) {
        for it in emojis {
            let u = it.get("url").and_then(|v| v.as_str()).unwrap_or("");
            if !u.is_empty() && !is_safe_img_url(u) {
                return write_error(
                    StatusCode::BAD_REQUEST,
                    "unsupported emoji url scheme (allowed: http, https)",
                );
            }
        }
    }

    let raw = req.settings.get().as_bytes().to_vec();
    if let Err(e) = write_file_atomic(&app.cfg.settings_path, &raw) {
        tracing::error!(error = %e, path = %app.cfg.settings_path, "settings: atomic write failed");
        return write_error(StatusCode::INTERNAL_SERVER_ERROR, "Server error");
    }

    let mod_time = std::fs::metadata(&app.cfg.settings_path)
        .ok()
        .and_then(|m| m.modified().ok())
        .unwrap_or(SystemTime::now());
    *app.settings_cache.lock().unwrap_or_else(|e| e.into_inner()) = Some((raw, mod_time));

    #[derive(serde::Serialize)]
    struct Ok_<'a> {
        status: &'a str,
    }
    write_json(StatusCode::OK, &Ok_ { status: "ok" })
}
