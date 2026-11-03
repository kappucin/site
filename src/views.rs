use askama::Template;
use axum::body::Body;
use axum::http::{header, StatusCode};
use axum::response::Response;

use crate::comments::Comment;
use crate::render;

#[derive(Clone)]
pub struct CommentView {
    pub id: String,
    pub name: String,
    pub html: String,
    pub date: String,
    pub date_fallback: String,
    pub device_icon: &'static str,
    pub device_title: String,
    pub is_admin: bool,
    pub is_owner: bool,
    pub user_vote: String,
    pub state_class: &'static str,
    pub score: i64,
}

pub(crate) fn state_class(user_vote: &str) -> &'static str {
    match user_vote {
        "up" => "state-up",
        "down" => "state-down",
        _ => "",
    }
}

pub(crate) fn comment_view(c: Comment, emojis: &[render::EmojiDef]) -> CommentView {
    let user_vote = if c.user_vote.is_empty() {
        "none".to_string()
    } else {
        c.user_vote.clone()
    };
    let cls = state_class(&user_vote);
    CommentView {
        html: render::parse_markdown(&c.text, emojis),
        device_icon: render::device_icon(&c.device),
        device_title: if c.device.is_empty() {
            "web".to_string()
        } else {
            c.device.clone()
        },
        id: c.id,
        name: c.name,
        date: c.date.to_string(),
        date_fallback: render::fmt_date_unix(c.date),
        is_admin: c.is_admin,
        is_owner: c.is_owner,
        user_vote,
        state_class: cls,
        score: c.score,
    }
}

#[derive(Clone)]
pub struct PostView {
    pub id: String,
    pub title: String,
    pub date: String,
    pub date_fallback: String,
    pub date_short: String,
    pub html: String,
    pub snippet: String,
    pub dropcap: bool,
}

pub(crate) fn post_views(
    posts: &[crate::blog::BlogPost],
    emojis: &[render::EmojiDef],
) -> Vec<PostView> {
    posts
        .iter()
        .map(|p| {
            let src = if p.text.is_empty() {
                p.title.as_str()
            } else {
                p.text.as_str()
            };
            PostView {
                id: p.id.clone(),
                title: p.title.clone(),
                date: p.date.clone(),
                date_fallback: render::fmt_blog_date(&p.date),
                date_short: render::fmt_blog_date_short(&p.date),
                html: render::render_post_markdown(src, emojis),
                snippet: render::render_snippet(src, emojis),
                dropcap: src.contains('\n') || src.trim().chars().count() > 65,
            }
        })
        .collect()
}

pub struct ContactView {
    pub icon: String,
    pub text: String,
    pub link: String,
    pub external: bool,
    pub corner: &'static str,
}

pub struct EmojiView {
    pub code: String,
    pub url: String,
}

#[derive(Template)]
#[template(path = "captcha.html")]
pub struct CaptchaTemplate {
    pub id: String,
    pub prefix: String,
    pub n: u32,
}

#[derive(Template)]
#[template(path = "comments_feed.html")]
pub struct CommentsFeedTemplate {
    pub comments: Vec<CommentView>,
    pub page: usize,
    pub pages: usize,
}

#[derive(Template)]
#[template(path = "page_indicator_oob.html")]
pub struct PageIndicatorOobTemplate {
    pub page: usize,
    pub pages: usize,
}

#[derive(Template)]
#[template(path = "vote_capsule.html")]
pub struct VoteCapsuleTemplate {
    pub c: CommentView,
}

#[derive(Template)]
#[template(path = "featured_comment_oob.html")]
pub struct FeaturedCommentOobTemplate {
    pub c: CommentView,
}

#[derive(Template)]
#[template(path = "blog_list.html")]
pub struct BlogListTemplate {
    pub lead: Option<PostView>,
    pub rest: Vec<PostView>,
}

#[derive(Template)]
#[template(path = "post_detail.html")]
pub struct PostDetailTemplate {
    pub p: PostView,
}

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {
    pub active_tab: &'static str,
    pub htmx_ver: String,
    pub app_ver: String,
    pub css_ver: String,
    pub theme_ver: String,
    pub og_title: String,
    pub og_description: String,
    pub og_url: String,
    pub contacts: Vec<ContactView>,
    pub emojis: Vec<EmojiView>,
    pub latest: Option<PostView>,
    pub post: Option<PostView>,
    pub lead: Option<PostView>,
    pub rest: Vec<PostView>,
    pub comments: Vec<CommentView>,
    pub page: usize,
    pub pages: usize,
    pub featured: Option<CommentView>,
}

pub(crate) fn html_response(status: StatusCode, body: String) -> Response {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(header::CACHE_CONTROL, "private, no-store")
        .body(Body::from(body))
        .unwrap_or_else(|_| Response::new(Body::empty()))
}
