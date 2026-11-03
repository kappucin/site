use std::sync::Arc;

use askama::Template;
use axum::extract::{Path, State};
use axum::http::{header, HeaderName, HeaderValue, StatusCode, Uri};
use axum::response::Response;
use axum::{Extension, Form};
use serde::Deserialize;

use crate::api::{detect_device, query_param, rune_count, voter_hash};
use crate::captcha;
use crate::comments;
use crate::db::{self, StoreErr};
use crate::mw::{AccountId, ClientIp};
use crate::render;
use crate::views::{
    comment_view, html_response, post_views, state_class, CaptchaTemplate, CommentView,
    CommentsFeedTemplate, FeaturedCommentOobTemplate, PageIndicatorOobTemplate, PostDetailTemplate,
    BlogListTemplate, VoteCapsuleTemplate,
};
use crate::App;

const COMMENTS_PER_PAGE: usize = 7;

fn decode_query(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
                match u8::from_str_radix(hex, 16) {
                    Ok(v) => {
                        out.push(v);
                        i += 3;
                    }
                    Err(_) => {
                        out.push(b'%');
                        i += 1;
                    }
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hx_error(status: StatusCode) -> Response {
    html_response(status, String::new())
}

#[derive(Clone)]
pub(crate) struct CommentsParams {
    sort: String,
    profanity: String,
    q: String,
    page: usize,
}

impl CommentsParams {
    pub(crate) fn default_params() -> Self {
        CommentsParams {
            sort: "new".to_string(),
            profanity: "hide".to_string(),
            q: String::new(),
            page: 1,
        }
    }

    fn from_uri(uri: &Uri) -> Self {
        let sort = if query_param(uri, "sort").as_deref() == Some("old") {
            "old"
        } else {
            "new"
        };
        let profanity = if query_param(uri, "profanity").as_deref() == Some("show") {
            "show"
        } else {
            "hide"
        };
        let q = decode_query(&query_param(uri, "q").unwrap_or_default());
        let page = query_param(uri, "page")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(1)
            .max(1);
        CommentsParams {
            sort: sort.to_string(),
            profanity: profanity.to_string(),
            q,
            page,
        }
    }
}

pub(crate) async fn comments_data(
    app: &App,
    account: &str,
    p: &CommentsParams,
    emojis: &[render::EmojiDef],
) -> Result<CommentsFeedTemplate, ()> {
    let voter = voter_hash(&app.cfg.voter_hash_salt, account);
    let account_db = account.to_string();
    let show_profanity = p.profanity == "show";
    let mut list = db::call(&app.db, move |conn| {
        comments::list_comments(conn, &voter, &account_db, show_profanity)
    })
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "comments_data: query failed");
    })?;
    let q = p.q.trim().to_lowercase();
    if !q.is_empty() {
        list.retain(|c| c.name.to_lowercase().contains(&q) || c.text.to_lowercase().contains(&q));
    }
    if p.sort == "old" {
        list.reverse();
    }
    let total = list.len();
    let pages = if total == 0 {
        1
    } else {
        (total + COMMENTS_PER_PAGE - 1) / COMMENTS_PER_PAGE
    };
    let page = p.page.min(pages).max(1);
    let comments: Vec<CommentView> = list
        .into_iter()
        .skip((page - 1) * COMMENTS_PER_PAGE)
        .take(COMMENTS_PER_PAGE)
        .map(|c| comment_view(c, emojis))
        .collect();
    Ok(CommentsFeedTemplate {
        comments,
        page,
        pages,
    })
}

async fn render_comments_response(app: &App, account: &str, p: &CommentsParams) -> Result<String, ()> {
    let emojis = render::load_site_settings(&app.cfg.settings_path).emojis;
    let tpl = comments_data(app, account, p, &emojis).await?;
    let mut body = tpl.render().map_err(|e| {
        tracing::error!(error = %e, "comments feed: template render failed");
    })?;
    let indicator = PageIndicatorOobTemplate {
        page: tpl.page,
        pages: tpl.pages,
    }
    .render()
    .map_err(|e| {
        tracing::error!(error = %e, "page indicator: template render failed");
    })?;
    body.push_str(&indicator);
    Ok(body)
}

pub async fn fragment_comments(
    State(app): State<Arc<App>>,
    Extension(AccountId(account)): Extension<AccountId>,
    uri: Uri,
) -> Response {
    let p = CommentsParams::from_uri(&uri);
    match render_comments_response(&app, &account, &p).await {
        Ok(body) => html_response(StatusCode::OK, body),
        Err(()) => hx_error(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

#[derive(Deserialize)]
pub struct CommentForm {
    #[serde(default)]
    name: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    captcha_id: String,
    #[serde(default)]
    pow_nonce: String,
    #[serde(default)]
    website: String,
    #[serde(default)]
    sort: String,
    #[serde(default)]
    profanity: String,
    #[serde(default)]
    q: String,
}

pub async fn fragment_post_comment(
    State(app): State<Arc<App>>,
    Extension(AccountId(account)): Extension<AccountId>,
    Extension(ClientIp(ip)): Extension<ClientIp>,
    headers: axum::http::HeaderMap,
    Form(form): Form<CommentForm>,
) -> Response {
    if !app.comment_limiter.allow(&ip) {
        return hx_error(StatusCode::TOO_MANY_REQUESTS);
    }
    let name = form.name.trim().to_string();
    let text = form.text.trim().to_string();
    if name.is_empty() || text.is_empty() {
        return hx_error(StatusCode::BAD_REQUEST);
    }
    if rune_count(&name) > comments::MAX_NAME_RUNES || rune_count(&text) > comments::MAX_TEXT_RUNES
    {
        return hx_error(StatusCode::BAD_REQUEST);
    }
    if !form.website.is_empty() {
        return hx_error(StatusCode::BAD_REQUEST);
    }
    if app.cfg.captcha_enabled {
        let id_db = form.captcha_id.clone();
        let nonce_db = form.pow_nonce.clone();
        let ttl = app.cfg.captcha_ttl;
        let ip_hash = voter_hash(&app.cfg.voter_hash_salt, &ip);
        let verified = db::call(&app.db, move |conn| {
            let ok = captcha::verify(conn, &id_db, &nonce_db, ttl)?;
            if ok {
                captcha::clear_fails(conn, &ip_hash);
            } else {
                captcha::bump_fails(conn, &ip_hash);
            }
            Ok(ok)
        })
        .await;
        match verified {
            Ok(true) => {}
            Ok(false) => return hx_error(StatusCode::BAD_REQUEST),
            Err(e) => {
                tracing::error!(error = %e, "captcha verify failed");
                return hx_error(StatusCode::BAD_REQUEST);
            }
        }
    }

    let device = detect_device(
        headers
            .get(header::USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .unwrap_or(""),
    );
    let name_db = name.clone();
    let text_db = text.clone();
    let account_db = account.clone();
    let created = db::call(&app.db, move |conn| {
        comments::create_comment(conn, &name_db, &text_db, device, &account_db)
    })
    .await;
    if let Err(e) = created {
        tracing::error!(error = %e, "create_comment: insert failed");
        return hx_error(StatusCode::INTERNAL_SERVER_ERROR);
    }

    let p = CommentsParams {
        sort: if form.sort == "old" { "old" } else { "new" }.to_string(),
        profanity: if form.profanity == "show" { "show" } else { "hide" }.to_string(),
        q: form.q,
        page: 1,
    };
    let feed = match render_comments_response(&app, &account, &p).await {
        Ok(body) => body,
        Err(()) => return hx_error(StatusCode::INTERNAL_SERVER_ERROR),
    };

    let featured = match render_featured_oob(&app, &account).await {
        Ok(body) => body,
        Err(()) => return hx_error(StatusCode::INTERNAL_SERVER_ERROR),
    };

    let mut body = feed;
    body.push_str(&featured);
    let mut res = html_response(StatusCode::OK, body);
    res.headers_mut().insert(
        HeaderName::from_static("hx-trigger"),
        HeaderValue::from_static("{\"kppcn:sent\":true}"),
    );
    res
}

async fn render_featured_oob(app: &App, account: &str) -> Result<String, ()> {
    let emojis = render::load_site_settings(&app.cfg.settings_path).emojis;
    let dp = CommentsParams::default_params();
    let tpl = comments_data(app, account, &dp, &emojis).await?;
    let c = tpl.comments.into_iter().next().ok_or(())?;
    FeaturedCommentOobTemplate { c }
        .render()
        .map_err(|e| {
            tracing::error!(error = %e, "featured oob: template render failed");
        })
}

pub async fn fragment_delete_comment(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    Extension(AccountId(account)): Extension<AccountId>,
    Extension(ClientIp(ip)): Extension<ClientIp>,
    uri: Uri,
) -> Response {
    if !app.delete_limiter.allow(&ip) {
        return hx_error(StatusCode::TOO_MANY_REQUESTS);
    }
    let id_db = id.clone();
    let account_db = account.clone();
    let res: Result<(), StoreErr> = db::call(&app.db, move |conn| {
        comments::delete_own(conn, &id_db, &account_db)
    })
    .await;
    match res {
        Ok(()) => {
            let p = CommentsParams::from_uri(&uri);
            match render_comments_response(&app, &account, &p).await {
                Ok(body) => html_response(StatusCode::OK, body),
                Err(()) => hx_error(StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
        Err(StoreErr::NotOwner) | Err(StoreErr::NotFound) => hx_error(StatusCode::FORBIDDEN),
        Err(e) => {
            tracing::error!(error = %e, "delete_comment failed");
            hx_error(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn fragment_captcha(
    State(app): State<Arc<App>>,
    Extension(AccountId(account)): Extension<AccountId>,
    Extension(ClientIp(ip)): Extension<ClientIp>,
) -> Response {
    if !app.cfg.captcha_enabled {
        return html_response(StatusCode::OK, String::new());
    }
    if !app.captcha_limiter.allow(&ip) {
        return hx_error(StatusCode::TOO_MANY_REQUESTS);
    }
    let ip_hash = voter_hash(&app.cfg.voter_hash_salt, &ip);
    let account_db = account.clone();
    let base_n = app.cfg.captcha_difficulty;
    let ttl = app.cfg.captcha_ttl;
    let issued = db::call(&app.db, move |conn| {
        captcha::issue(conn, &account_db, &ip_hash, base_n, ttl)
    })
    .await;
    let rendered = match issued {
        Ok(i) => CaptchaTemplate {
            id: i.id,
            prefix: i.prefix,
            n: i.n,
        }
        .render()
        .map_err(|e| {
            tracing::error!(error = %e, "captcha fragment render failed");
            StoreErr::Db("captcha render".into())
        }),
        Err(e) => {
            tracing::error!(error = %e, "captcha issue failed");
            Err(e)
        }
    };
    match rendered {
        Ok(body) => html_response(StatusCode::OK, body),
        Err(_) => hx_error(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn fragment_vote(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    Extension(AccountId(account)): Extension<AccountId>,
    Extension(ClientIp(ip)): Extension<ClientIp>,
    uri: Uri,
) -> Response {
    if !app.vote_limiter.allow(&ip) {
        return hx_error(StatusCode::TOO_MANY_REQUESTS);
    }
    let dir = query_param(&uri, "dir").unwrap_or_default();
    if dir != "up" && dir != "down" {
        return hx_error(StatusCode::BAD_REQUEST);
    }
    let voter = voter_hash(&app.cfg.voter_hash_salt, &account);
    let dir_up = dir == "up";
    let id_db = id.clone();
    let voter_db = voter.clone();
    let res: Result<(String, i64), StoreErr> = db::call(&app.db, move |conn| {
        comments::vote_toggle(conn, &id_db, &voter_db, dir_up)
    })
    .await;

    match res {
        Ok((user_vote, score)) => {
            let tpl = VoteCapsuleTemplate {
                c: CommentView {
                    id,
                    name: String::new(),
                    html: String::new(),
                    date: String::new(),
                    date_fallback: String::new(),
                    device_icon: "",
                    device_title: String::new(),
                    is_admin: false,
                    is_owner: false,
                    state_class: state_class(&user_vote),
                    user_vote,
                    score,
                },
            };
            match tpl.render() {
                Ok(body) => html_response(StatusCode::OK, body),
                Err(_) => hx_error(StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
        Err(StoreErr::NotFound) => hx_error(StatusCode::NOT_FOUND),
        Err(e) => {
            tracing::error!(error = %e, "vote failed");
            hx_error(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn load_blog_views(app: &App) -> Result<Vec<crate::views::PostView>, ()> {
    let emojis = render::load_site_settings(&app.cfg.settings_path).emojis;
    let posts = crate::blog::list_posts(&app.db).map_err(|e| {
        tracing::error!(error = %e, "fragment_blog: list_posts failed");
    })?;
    Ok(post_views(&posts, &emojis))
}

pub async fn fragment_blog(
    State(app): State<Arc<App>>,
    Extension(_account): Extension<AccountId>,
) -> Response {
    match load_blog_views(&app).await {
        Ok(posts) => {
            let tpl = BlogListTemplate {
                lead: posts.first().cloned(),
                rest: posts.iter().skip(1).cloned().collect(),
            };
            match tpl.render() {
                Ok(body) => html_response(StatusCode::OK, body),
                Err(e) => {
                    tracing::error!(error = %e, "fragment_blog: template render failed");
                    hx_error(StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        }
        Err(()) => hx_error(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn fragment_post(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    Extension(_account): Extension<AccountId>,
) -> Response {
    match load_blog_views(&app).await {
        Ok(posts) => match posts.into_iter().find(|p| p.id == id) {
            Some(p) => {
                let tpl = PostDetailTemplate { p };
                match tpl.render() {
                    Ok(body) => html_response(StatusCode::OK, body),
                    Err(e) => {
                        tracing::error!(error = %e, "fragment_post: template render failed");
                        hx_error(StatusCode::INTERNAL_SERVER_ERROR)
                    }
                }
            }
            None => hx_error(StatusCode::NOT_FOUND),
        },
        Err(()) => hx_error(StatusCode::INTERNAL_SERVER_ERROR),
    }
}
