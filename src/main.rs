mod account;
mod api;
mod blog;
mod captcha;
mod comments;
mod config;
mod db;
mod idgen;
mod mw;
mod fragments;
mod pages;
mod views;
mod profanity;
mod render;
mod settings;

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{header, HeaderValue, Method, StatusCode};
use axum::middleware as axum_middleware;
use axum::response::Response;
use axum::routing::{delete, get, post, put};
use axum::Router;
use sha2::{Digest, Sha256};
use tower::util::ServiceExt;
use tower_http::services::ServeDir;

use crate::mw::ClientIp;

pub struct App {
    pub cfg: config::Config,
    pub db: db::Db,
    pub settings_cache: Mutex<Option<(Vec<u8>, std::time::SystemTime)>>,
    pub comment_limiter: mw::RateLimiter,
    pub vote_limiter: mw::RateLimiter,
    pub delete_limiter: mw::RateLimiter,
    pub lockout: Mutex<std::collections::HashMap<String, mw::LockState>>,
    pub admin_user_hash: [u8; 32],
    pub admin_pass_hash: [u8; 32],
    pub captcha_limiter: mw::RateLimiter,
}

async fn static_admin(State(app): State<Arc<App>>, req: Request) -> Response {
    let ip = req
        .extensions()
        .get::<ClientIp>()
        .map(|c| c.0.clone())
        .unwrap_or_default();
    if let Err(resp) = mw::admin_check(&app, false, req.headers(), &ip) {
        return resp;
    }
    match ServeDir::new(&app.cfg.static_dir)
        .precompressed_br()
        .precompressed_gzip()
        .oneshot(req)
        .await
    {
        Ok(r) => r.map(Body::new),
        Err(_) => api::not_found_plain(),
    }
}

async fn spa_fallback(State(app): State<Arc<App>>, req: Request) -> Response {
    let path = req.uri().path().to_string();
    if path.starts_with("/api/") {
        return api::not_found_plain();
    }
    if path.starts_with("/admin.html") || path.starts_with("/js/admin.js") {
        let ip = req
            .extensions()
            .get::<ClientIp>()
            .map(|c| c.0.clone())
            .unwrap_or_default();
        if let Err(resp) = mw::admin_check(&app, false, req.headers(), &ip) {
            return resp;
        }
    }
    let is_asset = path.starts_with("/js/") || path.starts_with("/images/");
    let method = req.method().clone();

    if (method == Method::GET || method == Method::HEAD) && !is_asset {
        let rel = path.trim_start_matches('/');
        let fs_path = std::path::Path::new(&app.cfg.static_dir).join(rel);
        let is_file = !rel.is_empty()
            && !rel.contains("..")
            && std::fs::metadata(&fs_path).map(|m| m.is_file()).unwrap_or(false);
        if !is_file {
            let account = req
                .extensions()
                .get::<mw::AccountId>()
                .map(|a| a.0.clone())
                .unwrap_or_default();
            let mut res = pages::render_page(&app, &account, "home", None).await;
            if res.status() == StatusCode::OK {
                *res.status_mut() = StatusCode::NOT_FOUND;
            }
            res.headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
            return res;
        }
    }

    match ServeDir::new(&app.cfg.static_dir)
        .precompressed_br()
        .precompressed_gzip()
        .oneshot(req)
        .await
    {
        Ok(r) => r.map(Body::new),
        Err(_) => api::not_found_plain(),
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutdown signal received");
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cfg = match config::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("config error: {e}");
            std::process::exit(1);
        }
    };

    if let Some(parent) = std::path::Path::new(&cfg.db_path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let pool = match db::open(&cfg.db_path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("database error: {e}");
            std::process::exit(1);
        }
    };

    let app = Arc::new(App {
        admin_user_hash: Sha256::digest(cfg.admin_username.as_bytes()).into(),
        admin_pass_hash: Sha256::digest(cfg.admin_password.as_bytes()).into(),
        cfg,
        db: pool,
        settings_cache: Mutex::new(None),
        comment_limiter: mw::RateLimiter::new(std::time::Duration::from_secs(5)),
        vote_limiter: mw::RateLimiter::new(std::time::Duration::from_secs(1)),
        delete_limiter: mw::RateLimiter::new(std::time::Duration::from_secs(2)),
        captcha_limiter: mw::RateLimiter::new(std::time::Duration::from_secs(6)),
        lockout: Mutex::new(std::collections::HashMap::new()),
    });

    let router = Router::new()
        .route("/", get(pages::page_home))
        .route("/blog", get(pages::page_blog))
        .route("/comments", get(pages::page_comments))
        .route("/post/{id}", get(pages::page_post))
        .route(
            "/fragments/comments",
            get(fragments::fragment_comments).post(fragments::fragment_post_comment),
        )
        .route("/fragments/captcha", get(fragments::fragment_captcha))
        .route("/fragments/comments/{id}/vote", post(fragments::fragment_vote))
        .route(
            "/fragments/comments/{id}",
            delete(fragments::fragment_delete_comment),
        )
        .route("/fragments/blog", get(fragments::fragment_blog))
        .route("/fragments/post/{id}", get(fragments::fragment_post))
        .route("/api/settings", get(settings::get_settings))
        .route("/api/comments", get(comments::get_comments))
        .route("/api/blog", get(blog::get_blog))
        .route("/api/admin/settings", put(settings::admin_update_settings))
        .route(
            "/api/admin/comments/{id}",
            put(comments::admin_edit_comment).delete(comments::admin_delete_comment),
        )
        .route("/api/admin/blog", post(blog::admin_create))
        .route(
            "/api/admin/blog/{id}",
            put(blog::admin_update).delete(blog::admin_delete),
        )
        .route("/admin.html", get(static_admin))
        .route("/js/admin.js", get(static_admin))
        .fallback(spa_fallback)

        .layer(axum_middleware::from_fn_with_state(
            app.clone(),
            mw::account_and_ip_mw,
        ))
        .layer(axum_middleware::from_fn_with_state(
            app.clone(),
            mw::security_headers_mw,
        ))
        .with_state(app.clone());

    let addr = format!("0.0.0.0:{}", app.cfg.port);
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("bind {addr}: {e}");
            std::process::exit(1);
        }
    };
    tracing::info!("listening on {addr}");

    let serve = axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal());

    if let Err(e) = serve.await {
        eprintln!("server error: {e}");
        std::process::exit(1);
    }
    tracing::info!("server stopped");
}
