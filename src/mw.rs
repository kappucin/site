use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, Request, State};
use axum::http::{header, HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use base64::Engine;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::api::plain_detail;
use crate::App;

#[derive(Clone)]
pub struct AccountId(pub String);

#[derive(Clone)]
pub struct ClientIp(pub String);

pub async fn account_and_ip_mw(
    State(app): State<Arc<App>>,
    mut req: Request,
    next: Next,
) -> Response {

    let ip = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip().to_string())
        .unwrap_or_default();
    req.extensions_mut().insert(ClientIp(ip));

    let mut cookie_id: Option<String> = None;
    let acct_prefix = format!("{}=", crate::account::COOKIE_NAME);
    'outer: for h in req.headers().get_all(header::COOKIE) {
        if let Ok(s) = h.to_str() {
            for part in s.split(';') {
                let part = part.trim();
                if let Some(v) = part.strip_prefix(&acct_prefix) {
                    if let Some(id) =
                        crate::account::verify(app.cfg.account_secret.as_bytes(), v)
                    {
                        cookie_id = Some(id);
                        break 'outer;
                    }
                }
            }
        }
    }
    let (id, fresh) = match cookie_id {
        Some(id) => (id, false),
        None => (crate::account::new_account_id(), true),
    };
    let path = req.uri().path().to_string();
    req.extensions_mut().insert(AccountId(id.clone()));

    let mut res = next.run(req).await;

    if fresh {

        let skip = path.starts_with("/js/") || path.starts_with("/css/") || path.starts_with("/images/");
        if !skip {
            let mut c = format!(
                "{}={}; Path=/; Max-Age={}; HttpOnly; SameSite=Lax",
                crate::account::COOKIE_NAME,
                crate::account::sign(app.cfg.account_secret.as_bytes(), &id),
                crate::account::COOKIE_MAX_AGE
            );
            if app.cfg.secure_cookies {
                c.push_str("; Secure");
            }
            if let Ok(hv) = HeaderValue::from_str(&c) {
                res.headers_mut().append(header::SET_COOKIE, hv);
            }
        }
    }
    res
}

const CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob: https:; connect-src 'self'; worker-src 'self' blob:; font-src 'self'; object-src 'none'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

pub async fn security_headers_mw(
    State(app): State<Arc<App>>,
    req: Request,
    next: Next,
) -> Response {
    let path = req.uri().path().to_string();
    let mut res = next.run(req).await;
    let ok = res.status().is_success();
    let h = res.headers_mut();
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    h.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    h.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    if app.cfg.enable_hsts {
        h.insert(
            header::STRICT_TRANSPORT_SECURITY,
            HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        );
    }
    h.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(CSP),
    );
    h.insert(
        HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static("camera=(), microphone=(), geolocation=(), payment=()"),
    );
    h.insert(
        HeaderName::from_static("cross-origin-opener-policy"),
        HeaderValue::from_static("same-origin"),
    );

    if ok && path != "/js/admin.js" {
        if path.starts_with("/js/") || path.starts_with("/css/") {
            h.insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=31536000, immutable"),
            );
        } else if path.starts_with("/fonts/") {
            h.insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=31536000, immutable"),
            );
        } else if path == "/favicon.ico" {
            h.insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=86400"),
            );
        } else if path.starts_with("/images/") {
            h.insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=604800"),
            );
        }
    }
    res
}

const MAX_RATE_LIMIT_ENTRIES: usize = 50000;

pub struct RateLimiter {
    interval: Duration,
    map: Mutex<HashMap<String, Instant>>,
}

impl RateLimiter {
    pub fn new(interval: Duration) -> Self {
        RateLimiter {
            interval,
            map: Mutex::new(HashMap::new()),
        }
    }

    pub fn allow(&self, ip: &str) -> bool {
        let mut m = self.map.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        if let Some(last) = m.get(ip) {
            if now.duration_since(*last) < self.interval {
                return false;
            }
        }

        if !m.contains_key(ip) && m.len() >= MAX_RATE_LIMIT_ENTRIES {
            let interval = self.interval;
            m.retain(|_, t| now.duration_since(*t) < interval);
            if m.len() >= MAX_RATE_LIMIT_ENTRIES {
                if let Some(k) = m.keys().next().cloned() {
                    m.remove(&k);
                }
            }
        }
        m.insert(ip.to_string(), now);
        true
    }

    pub fn check(&self, ip: &str) -> Result<(), Response> {
        if self.allow(ip) {
            Ok(())
        } else {
            Err(plain_detail(
                StatusCode::TOO_MANY_REQUESTS,
                "too fast, wait a moment",
            ))
        }
    }
}

const MAX_LOCKOUT_ENTRIES: usize = 10000;

pub struct LockState {
    pub fails: u32,
    pub locked_until: Option<Instant>,
}

fn origin_host_matches(origin: &str, headers: &HeaderMap) -> bool {
    let host_part = origin.split("://").nth(1).unwrap_or("");
    let origin_host = host_part.split('/').next().unwrap_or("");
    if origin_host.is_empty() {
        return false;
    }
    if let Some(h) = headers.get(header::HOST).and_then(|v| v.to_str().ok()) {
        if origin_host.eq_ignore_ascii_case(h) {
            return true;
        }
    }
    false
}

fn check_basic(app: &App, headers: &HeaderMap) -> bool {
    let mut user_hash = [0u8; 32];
    let mut pass_hash = [0u8; 32];
    if let Some(auth) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    {
        if let Some(b64) = auth.strip_prefix("Basic ") {
            if let Ok(raw) = base64::engine::general_purpose::STANDARD.decode(b64.trim()) {
                if let Ok(s) = String::from_utf8(raw) {
                    if let Some((u, p)) = s.split_once(':') {
                        user_hash = Sha256::digest(u.as_bytes()).into();
                        pass_hash = Sha256::digest(p.as_bytes()).into();
                    }
                }
            }
        }
    }

    let vu = bool::from(user_hash.ct_eq(&app.admin_user_hash));
    let vp = bool::from(pass_hash.ct_eq(&app.admin_pass_hash));
    vu && vp
}

pub fn admin_check(
    app: &App,
    mutating: bool,
    headers: &HeaderMap,
    ip: &str,
) -> Result<(), Response> {
    if mutating {
        let xrw = headers
            .get("X-Requested-With")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if xrw != "XMLHttpRequest" {
            return Err(plain_detail(
                StatusCode::FORBIDDEN,
                "csrf check failed",
            ));
        }
        if let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
            if !origin.is_empty()
                && !origin_host_matches(origin, headers)
            {
                return Err(plain_detail(
                    StatusCode::FORBIDDEN,
                    "csrf check failed",
                ));
            }
        }
    }

    {
        let mut m = app.lockout.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        if let Some(st) = m.get_mut(ip) {
            if let Some(until) = st.locked_until {
                if now < until {
                    return Err(plain_detail(
                        StatusCode::TOO_MANY_REQUESTS,
                        "too many login attempts, try again later",
                    ));
                }
                st.fails = 0;
                st.locked_until = None;
            }
        }
    }

    if !check_basic(app, headers) {
        let mut m = app.lockout.lock().unwrap_or_else(|e| e.into_inner());
        if !m.contains_key(ip) && m.len() >= MAX_LOCKOUT_ENTRIES {
            let now = Instant::now();

            m.retain(|_, v| v.locked_until.map_or(false, |u| now < u));
            if m.len() >= MAX_LOCKOUT_ENTRIES {
                if let Some(k) = m.keys().next().cloned() {
                    m.remove(&k);
                }
            }
        }
        let st = m.entry(ip.to_string()).or_insert(LockState {
            fails: 0,
            locked_until: None,
        });
        st.fails += 1;
        if st.fails >= 5 {
            st.locked_until = Some(Instant::now() + Duration::from_secs(600));
        }
        let mut resp = plain_detail(
            StatusCode::UNAUTHORIZED,
            "invalid credentials",
        );
        resp.headers_mut().insert(
            header::WWW_AUTHENTICATE,
            HeaderValue::from_static("Basic realm=\"admin\""),
        );
        return Err(resp);
    }

    app.lockout.lock().unwrap_or_else(|e| e.into_inner()).remove(ip);
    Ok(())
}
