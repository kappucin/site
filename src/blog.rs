use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use axum::Extension;
use redb::{Database, ReadableTable};
use serde::{Deserialize, Serialize};

use crate::api::{now_unix, rune_count, truncate_runes, write_error, write_json};
use crate::db::{self, dbe, BlogRow, StoreErr};
use crate::mw::{admin_check, ClientIp};
use crate::App;

const MAX_DATE_RUNES: usize = 100;
const MAX_TITLE_RUNES: usize = 300;
const MAX_TEXT_RUNES: usize = 100000;
const MAX_BLOG_POSTS_RETURNED: usize = 300;

#[derive(Serialize, Clone)]
pub struct BlogPost {
    pub id: String,
    pub title: String,
    pub text: String,
    pub date: String,
}

pub fn list_posts(db: &Database) -> Result<Vec<BlogPost>, StoreErr> {
    let txn = db.begin_read().map_err(dbe)?;
    let t = txn.open_table(db::BLOG_POSTS).map_err(dbe)?;

    let mut rows: Vec<BlogRow> = Vec::new();
    for entry in t.iter().map_err(dbe)? {
        let (_, v) = entry.map_err(dbe)?;
        rows.push(serde_json::from_str(v.value()).map_err(dbe)?);
    }
    rows.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    rows.truncate(MAX_BLOG_POSTS_RETURNED);

    Ok(rows
        .into_iter()
        .map(|r| BlogPost { id: r.id, title: r.title, text: r.text, date: r.date })
        .collect())
}

pub async fn get_blog(State(app): State<Arc<App>>) -> Response {
    match db::call(&app.db, |conn| list_posts(conn)).await {
        Ok(posts) => write_json(StatusCode::OK, &posts),
        Err(e) => {
            tracing::error!(error = %e, "get_blog: query failed");
            write_error(StatusCode::INTERNAL_SERVER_ERROR, "Server error")
        }
    }
}

#[derive(Deserialize)]
struct AdminBlogPostRequest {
    #[serde(default)]
    title: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    date: String,
}

fn validate_post(title: &str, text: &str) -> bool {
    rune_count(title) <= MAX_TITLE_RUNES && rune_count(text) <= MAX_TEXT_RUNES
}

pub async fn admin_create(
    State(app): State<Arc<App>>,
    Extension(ClientIp(ip)): Extension<ClientIp>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if let Err(resp) = admin_check(&app, true, &headers, &ip) {
        return resp;
    }
    let req: AdminBlogPostRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(_) => return write_error(StatusCode::BAD_REQUEST, "bad request"),
    };

    let title = req.title.trim().to_string();
    let text = req.text.trim().to_string();
    if title.is_empty() || text.is_empty() {
        return write_error(StatusCode::BAD_REQUEST, "empty title or text");
    }
    if !validate_post(&title, &text) {
        return write_error(
            StatusCode::BAD_REQUEST,
            "title or text too long",
        );
    }
    let date_str = truncate_runes(&req.date, MAX_DATE_RUNES).to_string();

    let post = BlogPost {
        id: crate::idgen::new_id(),
        title,
        text,
        date: date_str,
    };

    let row = BlogRow {
        id: post.id.clone(),
        title: post.title.clone(),
        text: post.text.clone(),
        date: post.date.clone(),
        created_at: now_unix(),
    };
    let res: Result<(), StoreErr> = db::call(&app.db, move |conn| {
        let json = serde_json::to_string(&row).map_err(dbe)?;
        let txn = conn.begin_write().map_err(dbe)?;
        {
            let mut t = txn.open_table(db::BLOG_POSTS).map_err(dbe)?;
            t.insert(row.id.as_str(), json.as_str()).map_err(dbe)?;
        }
        txn.commit().map_err(dbe)?;
        Ok(())
    })
    .await;
    if let Err(e) = res {
        tracing::error!(error = %e, "blog admin_create: insert failed");
        return write_error(StatusCode::INTERNAL_SERVER_ERROR, "Server error");
    }

    write_json(
        StatusCode::CREATED,
        &serde_json::json!({"status": "success", "post": post}),
    )
}

pub async fn admin_update(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    Extension(ClientIp(ip)): Extension<ClientIp>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if let Err(resp) = admin_check(&app, true, &headers, &ip) {
        return resp;
    }
    let req: AdminBlogPostRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(_) => return write_error(StatusCode::BAD_REQUEST, "bad request"),
    };

    let title = req.title.trim().to_string();
    let text = req.text.trim().to_string();
    if title.is_empty() || text.is_empty() {
        return write_error(StatusCode::BAD_REQUEST, "empty title or text");
    }
    if !validate_post(&title, &text) {
        return write_error(
            StatusCode::BAD_REQUEST,
            "title or text too long",
        );
    }

    let res: Result<(), StoreErr> = db::call(&app.db, move |conn| {
        let txn = conn.begin_write().map_err(dbe)?;
        {
            let mut t = txn.open_table(db::BLOG_POSTS).map_err(dbe)?;
            let raw = t.get(id.as_str()).map_err(dbe)?.map(|g| g.value().to_string());
            let mut row: BlogRow = match raw {
                Some(s) => serde_json::from_str(&s).map_err(dbe)?,
                None => return Err(StoreErr::NotFound),
            };
            row.title = title;
            row.text = text;
            let json = serde_json::to_string(&row).map_err(dbe)?;
            t.insert(id.as_str(), json.as_str()).map_err(dbe)?;
        }
        txn.commit().map_err(dbe)?;
        Ok(())
    })
    .await;

    match res {
        Ok(()) => write_json(StatusCode::OK, &serde_json::json!({"status": "ok"})),
        Err(StoreErr::NotFound) => write_error(StatusCode::NOT_FOUND, "post not found"),
        Err(e) => {
            tracing::error!(error = %e, "blog admin_update failed");
            write_error(StatusCode::INTERNAL_SERVER_ERROR, "Server error")
        }
    }
}

pub async fn admin_delete(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    Extension(ClientIp(ip)): Extension<ClientIp>,
    headers: HeaderMap,
) -> Response {
    if let Err(resp) = admin_check(&app, true, &headers, &ip) {
        return resp;
    }
    let res: Result<(), StoreErr> = db::call(&app.db, move |conn| {
        let txn = conn.begin_write().map_err(dbe)?;
        {
            let mut t = txn.open_table(db::BLOG_POSTS).map_err(dbe)?;
            t.remove(id.as_str()).map_err(dbe)?;
        }
        txn.commit().map_err(dbe)?;
        Ok(())
    })
    .await;

    match res {
        Ok(()) => write_json(StatusCode::OK, &serde_json::json!({"status": "ok"})),
        Err(e) => {
            tracing::error!(error = %e, "blog admin_delete failed");
            write_error(StatusCode::INTERNAL_SERVER_ERROR, "Server error")
        }
    }
}
