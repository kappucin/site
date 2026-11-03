use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use askama::Template;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::Extension;
use sha2::{Digest, Sha256};

use crate::fragments::{comments_data, CommentsParams};
use crate::mw::AccountId;
use crate::render;
use crate::views::{
    html_response, post_views, CommentsFeedTemplate, ContactView, EmojiView, IndexTemplate,
    PostView,
};
use crate::App;

fn asset_ver(app: &App, rel: &str) -> String {
    static CACHE: OnceLock<Mutex<HashMap<String, (SystemTime, String)>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let path = std::path::Path::new(&app.cfg.static_dir).join(rel);
    let mtime = std::fs::metadata(&path)
        .and_then(|m| m.modified())
        .unwrap_or(UNIX_EPOCH);
    {
        let c = cache.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((t, v)) = c.get(rel) {
            if *t == mtime {
                return v.clone();
            }
        }
    }
    let ver = match std::fs::read(&path) {
        Ok(bytes) => Sha256::digest(&bytes)[..4]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
        Err(_) => "0".to_string(),
    };
    cache
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(rel.to_string(), (mtime, ver.clone()));
    ver
}

pub async fn render_page(app: &App, account: &str, tab: &'static str, post_id: Option<&str>) -> Response {
    let settings = render::load_site_settings(&app.cfg.settings_path);

    let contacts: Vec<ContactView> = settings
        .contacts
        .iter()
        .filter(|c| !c.text.is_empty() && !c.link.is_empty())
        .map(|c| {
            let mut rb = [0u8; 1];
            let _ = getrandom::getrandom(&mut rb);
            let corner = ["tl", "tr", "bl", "br"][(rb[0] % 4) as usize];
            ContactView {
                icon: if c.icon.is_empty() {
                    render::contact_icon(&c.link).to_string()
                } else {
                    c.icon.clone()
                },
                text: c.text.clone(),
                link: render::safe_link(&c.link),
                external: render::is_external(&c.link),
                corner,
            }
        })
        .collect();

    let emoji_defs = settings.emojis;
    let emojis: Vec<EmojiView> = emoji_defs
        .iter()
        .filter(|e| !e.code.is_empty() && !e.url.is_empty())
        .map(|e| EmojiView {
            code: e.code.clone(),
            url: e.url.clone(),
        })
        .collect();

    let posts_raw = crate::blog::list_posts(&app.db).unwrap_or_else(|e| {
        tracing::error!(error = %e, "render_page: list_posts failed");
        Vec::new()
    });
    let posts: Vec<PostView> = post_views(&posts_raw, &emoji_defs);
    let latest = posts.first().cloned();
    let lead = posts.first().cloned();
    let rest: Vec<PostView> = posts.iter().skip(1).cloned().collect();

    let (post, not_found) = match post_id {
        Some(pid) => match posts.iter().find(|p| p.id == pid) {
            Some(p) => (Some(p.clone()), false),
            None => (None, true),
        },
        None => (None, false),
    };

    let (og_title, og_description, og_url) = match &post {
        Some(p) => {
            let mut plain = String::new();
            let mut in_tag = false;
            for ch in p.html.chars() {
                match ch {
                    '<' => in_tag = true,
                    '>' => in_tag = false,
                    c if !in_tag => plain.push(c),
                    _ => {}
                }
            }
            let mut desc = String::new();
            for ch in plain.chars() {
                if ch.is_whitespace() && desc.ends_with(' ') {
                    continue;
                }
                desc.push(ch);
            }
            let desc = desc.trim().to_string();
            let mut desc: String = desc.chars().take(160).collect();
            if plain.trim().chars().count() > 160 {
                desc.push('…');
            }
            (
                p.title.clone(),
                desc,
                format!("https://kppcn.cyou/post/{}", p.id),
            )
        }
        None => (String::new(), String::new(), String::new()),
    };

    let dp = CommentsParams::default_params();
    let cb = comments_data(app, account, &dp, &emoji_defs)
        .await
        .unwrap_or(CommentsFeedTemplate {
            comments: Vec::new(),
            page: 1,
            pages: 1,
        });
    let featured = cb.comments.first().cloned();

    let tpl = IndexTemplate {
        active_tab: if not_found { "blog" } else { tab },
        htmx_ver: asset_ver(app, "js/htmx.js"),
        app_ver: asset_ver(app, "js/app.js"),
        css_ver: asset_ver(app, "css/style.css"),
        theme_ver: asset_ver(app, "js/theme.js"),
        og_title,
        og_description,
        og_url,
        contacts,
        emojis,
        latest,
        post,
        lead,
        rest,
        comments: cb.comments,
        page: cb.page,
        pages: cb.pages,
        featured,
    };
    match tpl.render() {
        Ok(body) => html_response(
            if not_found { StatusCode::NOT_FOUND } else { StatusCode::OK },
            body,
        ),
        Err(e) => {
            tracing::error!(error = %e, "render_page: template render failed");
            crate::api::plain_detail(StatusCode::INTERNAL_SERVER_ERROR, "Server error")
        }
    }
}

pub async fn page_home(
    State(app): State<Arc<App>>,
    Extension(AccountId(account)): Extension<AccountId>,
) -> Response {
    render_page(&app, &account, "home", None).await
}

pub async fn page_blog(
    State(app): State<Arc<App>>,
    Extension(AccountId(account)): Extension<AccountId>,
) -> Response {
    render_page(&app, &account, "blog", None).await
}

pub async fn page_comments(
    State(app): State<Arc<App>>,
    Extension(AccountId(account)): Extension<AccountId>,
) -> Response {
    render_page(&app, &account, "comments", None).await
}

pub async fn page_post(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    Extension(AccountId(account)): Extension<AccountId>,
) -> Response {
    render_page(&app, &account, "blog", Some(&id)).await
}
