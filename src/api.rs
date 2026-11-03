use axum::body::Body;
use axum::http::{header, StatusCode, Uri};
use axum::response::Response;
use serde::Serialize;
use sha2::{Digest, Sha256};

pub fn write_json<T: Serialize>(status: StatusCode, data: &T) -> Response {
    let mut body = serde_json::to_vec(data).unwrap_or_default();
    body.push(b'\n');
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
        .header(header::CACHE_CONTROL, "private, no-store")
        .body(Body::from(body))
        .unwrap()
}

pub fn write_error(status: StatusCode, msg: &str) -> Response {
    #[derive(Serialize)]
    struct E<'a> {
        error: &'a str,
    }
    write_json(status, &E { error: msg })
}

pub fn plain_detail(status: StatusCode, detail: &str) -> Response {

    let mut body = serde_json::to_vec(&serde_json::json!({ "detail": detail }))
        .unwrap_or_else(|_| b"{\"detail\":\"error\"}".to_vec());
    body.push(b'\n');
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .body(Body::from(body))
        .unwrap()
}

pub fn not_found_plain() -> Response {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .body(Body::from("404 page not found\n"))
        .unwrap()
}

pub fn detect_device(ua: &str) -> &'static str {
    let ua = ua.trim().to_lowercase();
    if ua.is_empty() {
        return "unknown";
    }
    if ua.contains("iphone") || ua.contains("ipad") {
        return "iphone";
    }
    if ua.contains("mac os") {
        return "macos";
    }
    if ua.contains("android") {
        return "android";
    }
    if ua.contains("windows") {
        return "windows";
    }
    if ua.contains("linux") {
        return "linux";
    }
    "desktop"
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn voter_hash(salt: &str, account_id: &str) -> String {
    let d = Sha256::digest(format!("{salt}|{account_id}").as_bytes());
    hex(&d)
}

pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn truncate_runes(s: &str, n: usize) -> &str {
    match s.char_indices().nth(n) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

pub fn rune_count(s: &str) -> usize {
    s.chars().count()
}

pub fn query_param(uri: &Uri, key: &str) -> Option<String> {
    let q = uri.query()?;
    for pair in q.split('&') {
        let mut it = pair.splitn(2, '=');
        if it.next().unwrap_or("") == key {
            return Some(it.next().unwrap_or("").to_string());
        }
    }
    None
}
