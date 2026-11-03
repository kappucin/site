use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, Uri};
use axum::response::Response;
use axum::Extension;
use redb::{Database, ReadableTable};
use serde::{Deserialize, Serialize};

use crate::api::{now_unix, rune_count, voter_hash, write_error, write_json};
use crate::db::{self, dbe, vote_key, CommentRow, StoreErr};
use crate::mw::{admin_check, AccountId, ClientIp};
use crate::App;

pub const MAX_NAME_RUNES: usize = 80;
pub const MAX_TEXT_RUNES: usize = 4000;
const MAX_COMMENTS_RETURNED: usize = 500;
const MIN_SCORE: i64 = -1000000;
const MAX_SCORE: i64 = 1000000;

const ALLOWED_DEVICES: &[&str] = &["unknown", "iphone", "macos", "android", "windows", "linux", "desktop"];

#[derive(Serialize, Clone)]
pub struct Comment {
    pub id: String,
    pub name: String,
    pub text: String,
    pub date: i64,
    pub score: i64,
    pub is_admin: bool,
    pub device: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub user_vote: String,
    pub is_owner: bool,
}

pub fn list_comments(
    db: &Database,
    voter: &str,
    account_id: &str,
    show_profanity: bool,
) -> Result<Vec<Comment>, StoreErr> {
    let txn = db.begin_read().map_err(dbe)?;
    let comments_t = txn.open_table(db::COMMENTS).map_err(dbe)?;
    let votes_t = txn.open_table(db::COMMENT_VOTES).map_err(dbe)?;

    let mut rows: Vec<CommentRow> = Vec::new();
    for entry in comments_t.iter().map_err(dbe)? {
        let (_, v) = entry.map_err(dbe)?;
        rows.push(serde_json::from_str(v.value()).map_err(dbe)?);
    }
    rows.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    rows.truncate(MAX_COMMENTS_RETURNED);

    let mut comments = Vec::with_capacity(rows.len());
    for row in rows {
        let user_vote = if voter.is_empty() {
            "none".to_string()
        } else {
            votes_t
                .get(vote_key(&row.id, voter).as_str())
                .map_err(dbe)?
                .map(|g| g.value().to_string())
                .unwrap_or_else(|| "none".to_string())
        };
        let is_owner = !account_id.is_empty() && row.author_account == account_id;
        let (name, text) = if show_profanity {
            (row.name, row.text)
        } else {
            (crate::profanity::censor(&row.name), crate::profanity::censor(&row.text))
        };
        comments.push(Comment {
            id: row.id,
            name,
            text,
            date: row.created_at,
            score: row.score,
            is_admin: row.is_admin,
            device: row.device,
            user_vote,
            is_owner,
        });
    }
    Ok(comments)
}

pub fn create_comment(
    db: &Database,
    name: &str,
    text: &str,
    device: &str,
    account: &str,
) -> Result<Comment, StoreErr> {
    let now = now_unix();
    let row = CommentRow {
        id: crate::idgen::new_id(),
        name: name.to_string(),
        text: text.to_string(),
        score: 0,
        is_admin: false,
        device: device.to_string(),
        author_account: account.to_string(),
        created_at: now,
    };
    let json = serde_json::to_string(&row).map_err(dbe)?;

    let txn = db.begin_write().map_err(dbe)?;
    {
        let mut t = txn.open_table(db::COMMENTS).map_err(dbe)?;
        t.insert(row.id.as_str(), json.as_str()).map_err(dbe)?;
    }
    txn.commit().map_err(dbe)?;

    Ok(Comment {
        id: row.id,
        name: row.name,
        text: row.text,
        date: row.created_at,
        score: 0,
        is_admin: false,
        device: row.device,
        user_vote: String::new(),
        is_owner: true,
    })
}

pub fn delete_own(db: &Database, id: &str, account: &str) -> Result<(), StoreErr> {
    if account.is_empty() {
        return Err(StoreErr::NotOwner);
    }
    let txn = db.begin_write().map_err(dbe)?;
    {
        let mut comments_t = txn.open_table(db::COMMENTS).map_err(dbe)?;
        let raw = comments_t
            .get(id)
            .map_err(dbe)?
            .map(|g| g.value().to_string());
        let row: CommentRow = match raw {
            Some(s) => serde_json::from_str(&s).map_err(dbe)?,
            None => return Err(StoreErr::NotFound),
        };
        if row.author_account != account {
            return Err(StoreErr::NotOwner);
        }
        comments_t.remove(id).map_err(dbe)?;

        let mut votes_t = txn.open_table(db::COMMENT_VOTES).map_err(dbe)?;
        let prefix = format!("{id}\u{0}");
        let keys: Vec<String> = votes_t
            .iter()
            .map_err(dbe)?
            .filter_map(|e| e.ok())
            .filter(|(k, _)| k.value().starts_with(&prefix))
            .map(|(k, _)| k.value().to_string())
            .collect();
        for k in keys {
            votes_t.remove(k.as_str()).map_err(dbe)?;
        }
    }
    txn.commit().map_err(dbe)?;
    Ok(())
}

pub fn vote_toggle(
    db: &Database,
    id: &str,
    voter: &str,
    dir_up: bool,
) -> Result<(String, i64), StoreErr> {
    let txn = db.begin_write().map_err(dbe)?;
    let (new_state, new_score) = {
        let mut comments_t = txn.open_table(db::COMMENTS).map_err(dbe)?;
        let mut votes_t = txn.open_table(db::COMMENT_VOTES).map_err(dbe)?;

        let raw = comments_t
            .get(id)
            .map_err(dbe)?
            .map(|g| g.value().to_string());
        let mut row: CommentRow = match raw {
            Some(s) => serde_json::from_str(&s).map_err(dbe)?,
            None => return Err(StoreErr::NotFound),
        };

        let key = vote_key(id, voter);
        let old_state = votes_t
            .get(key.as_str())
            .map_err(dbe)?
            .map(|g| g.value().to_string())
            .unwrap_or_else(|| "none".to_string());

        let new_state = if dir_up {
            if old_state == "up" { "none" } else { "up" }
        } else if old_state == "down" {
            "none"
        } else {
            "down"
        };

        fn value(s: &str) -> i64 {
            match s {
                "up" => 1,
                "down" => -1,
                _ => 0,
            }
        }
        let delta = value(new_state) - value(&old_state);

        if new_state == "none" {
            votes_t.remove(key.as_str()).map_err(dbe)?;
        } else {
            votes_t.insert(key.as_str(), new_state).map_err(dbe)?;
        }

        if delta != 0 {
            row.score = (row.score + delta).clamp(MIN_SCORE, MAX_SCORE);
            let json = serde_json::to_string(&row).map_err(dbe)?;
            comments_t.insert(id, json.as_str()).map_err(dbe)?;
        }

        (new_state.to_string(), row.score)
    };
    txn.commit().map_err(dbe)?;
    Ok((new_state, new_score))
}

pub async fn get_comments(
    State(app): State<Arc<App>>,
    Extension(AccountId(account)): Extension<AccountId>,
    uri: Uri,
) -> Response {
    let show_profanity =
        crate::api::query_param(&uri, "profanity").as_deref() == Some("show");
    let voter = voter_hash(&app.cfg.voter_hash_salt, &account);
    match db::call(&app.db, move |conn| {
        list_comments(conn, &voter, &account, show_profanity)
    })
    .await
    {
        Ok(list) => write_json(StatusCode::OK, &list),
        Err(e) => {
            tracing::error!(error = %e, "get_comments: query failed");
            write_error(StatusCode::INTERNAL_SERVER_ERROR, "Server error")
        }
    }
}

#[derive(Deserialize)]
struct AdminCommentRequest {
    #[serde(default)]
    name: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    is_admin: bool,
    #[serde(default)]
    score: i64,
    #[serde(default)]
    device: String,
}

pub async fn admin_edit_comment(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    Extension(ClientIp(ip)): Extension<ClientIp>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if let Err(resp) = admin_check(&app, true, &headers, &ip) {
        return resp;
    }
    let req: AdminCommentRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(_) => return write_error(StatusCode::BAD_REQUEST, "bad request"),
    };

    let name = req.name.trim().to_string();
    let text = req.text.trim().to_string();
    if name.is_empty() || text.is_empty() {
        return write_error(
            StatusCode::BAD_REQUEST,
            "name and text cannot be empty",
        );
    }
    if rune_count(&name) > MAX_NAME_RUNES || rune_count(&text) > MAX_TEXT_RUNES {
        return write_error(StatusCode::BAD_REQUEST, "name or text too long");
    }
    if req.score < MIN_SCORE || req.score > MAX_SCORE {
        return write_error(StatusCode::BAD_REQUEST, "invalid score value");
    }
    let device = if ALLOWED_DEVICES.contains(&req.device.as_str()) {
        req.device.clone()
    } else {
        "unknown".to_string()
    };

    let res: Result<(), StoreErr> = db::call(&app.db, move |conn| {
        let txn = conn.begin_write().map_err(dbe)?;
        {
            let mut t = txn.open_table(db::COMMENTS).map_err(dbe)?;
            let raw = t.get(id.as_str()).map_err(dbe)?.map(|g| g.value().to_string());
            let mut row: CommentRow = match raw {
                Some(s) => serde_json::from_str(&s).map_err(dbe)?,
                None => return Err(StoreErr::NotFound),
            };
            row.name = name;
            row.text = text;
            row.is_admin = req.is_admin;
            row.score = req.score;
            row.device = device;
            let json = serde_json::to_string(&row).map_err(dbe)?;
            t.insert(id.as_str(), json.as_str()).map_err(dbe)?;
        }
        txn.commit().map_err(dbe)?;
        Ok(())
    })
    .await;

    match res {
        Ok(()) => write_json(StatusCode::OK, &serde_json::json!({"status": "ok"})),
        Err(StoreErr::NotFound) => write_error(StatusCode::NOT_FOUND, "comment not found"),
        Err(e) => {
            tracing::error!(error = %e, "admin_edit_comment failed");
            write_error(StatusCode::INTERNAL_SERVER_ERROR, "Server error")
        }
    }
}

pub async fn admin_delete_comment(
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
            let mut t = txn.open_table(db::COMMENTS).map_err(dbe)?;
            t.remove(id.as_str()).map_err(dbe)?;
            let mut votes_t = txn.open_table(db::COMMENT_VOTES).map_err(dbe)?;
            let prefix = format!("{id}\u{0}");
            let keys: Vec<String> = votes_t
                .iter()
                .map_err(dbe)?
                .filter_map(|e| e.ok())
                .filter(|(k, _)| k.value().starts_with(&prefix))
                .map(|(k, _)| k.value().to_string())
                .collect();
            for k in keys {
                votes_t.remove(k.as_str()).map_err(dbe)?;
            }
        }
        txn.commit().map_err(dbe)?;
        Ok(())
    })
    .await;

    match res {
        Ok(()) => write_json(StatusCode::OK, &serde_json::json!({"status": "ok"})),
        Err(e) => {
            tracing::error!(error = %e, "admin_delete_comment failed");
            write_error(StatusCode::INTERNAL_SERVER_ERROR, "Server error")
        }
    }
}
