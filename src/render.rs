use std::sync::OnceLock;

use pulldown_cmark::{html, Event, Options, Parser};
use regex::Regex;
use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct EmojiDef {
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub url: String,
}

#[derive(Deserialize, Clone)]
pub struct ContactDef {
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub link: String,
}

#[derive(Deserialize, Default, Clone)]
pub struct SiteSettings {
    #[serde(default)]
    pub contacts: Vec<ContactDef>,
    #[serde(default)]
    pub emojis: Vec<EmojiDef>,
}

pub fn load_site_settings(path: &str) -> SiteSettings {
    std::fs::read(path)
        .ok()
        .and_then(|d| serde_json::from_slice(&d).ok())
        .unwrap_or_default()
}

pub fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\'' => out.push_str("&#39;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

fn re(cell: &'static OnceLock<Regex>, pattern: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pattern).expect("static regex"))
}

static WS_RE: OnceLock<Regex> = OnceLock::new();

pub fn collapse_ws(text: &str) -> String {
    re(&WS_RE, r"\s+").replace_all(text, " ").trim().to_string()
}

static EMOJI_CODE_RE: OnceLock<Regex> = OnceLock::new();

fn replace_emojis(p: String, emojis: &[EmojiDef]) -> String {
    if emojis.is_empty() {
        return p;
    }
    let re = re(&EMOJI_CODE_RE, r":([\w-]+):");
    re.replace_all(&p, |caps: &regex::Captures| {
        let code = &caps[1];
        match emojis
            .iter()
            .find(|e| !e.code.is_empty() && !e.url.is_empty() && e.code == code)
        {
            Some(e) => format!(
                "<img src=\"{}\" class=\"inline-emoji\" alt=\":{}:\" loading=\"lazy\">",
                escape_html(&e.url),
                escape_html(code)
            ),
            None => caps[0].to_string(),
        }
    })
    .into_owned()
}

pub fn parse_markdown(text: &str, emojis: &[EmojiDef]) -> String {
    static BOLD: OnceLock<Regex> = OnceLock::new();
    static ITALIC: OnceLock<Regex> = OnceLock::new();
    static STRIKE: OnceLock<Regex> = OnceLock::new();
    static CODE: OnceLock<Regex> = OnceLock::new();
    static QUOTE: OnceLock<Regex> = OnceLock::new();

    let p = replace_emojis(escape_html(text), emojis);
    let p = re(&BOLD, r"\*\*(.*?)\*\*").replace_all(&p, "<span class=\"md-bold\">${1}</span>");
    let p = re(&ITALIC, r"\*(.*?)\*").replace_all(&p, "<span class=\"md-italic\">${1}</span>");
    let p = re(&STRIKE, r"~~(.*?)~~").replace_all(&p, "<span class=\"md-strike\">${1}</span>");
    let p = re(&CODE, r"`(.*?)`").replace_all(&p, "<span class=\"md-code\">${1}</span>");
    let p = re(&QUOTE, r"(?m)^&gt; (.*)$").replace_all(&p, "<div class=\"md-quote\">${1}</div>");
    p.replace('\n', "<br>")
}

pub fn render_post_markdown(text: &str, emojis: &[EmojiDef]) -> String {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TABLES);
    let events = Parser::new_ext(text, opts).map(|ev| match ev {
        Event::Html(h) => Event::Text(h),
        Event::InlineHtml(h) => Event::Text(h),
        Event::Text(t) => Event::Html(replace_emojis(escape_html(&t), emojis).into()),
        Event::SoftBreak => Event::HardBreak,
        _ => ev,
    });
    let mut out = String::with_capacity(text.len() * 2);
    html::push_html(&mut out, events);
    out
}

pub fn render_snippet(text: &str, emojis: &[EmojiDef]) -> String {
    replace_emojis(escape_html(&collapse_ws(text)), emojis)
}

pub fn safe_link(link: &str) -> String {
    if crate::settings::is_safe_link(link) {
        link.to_string()
    } else {
        "#".to_string()
    }
}

pub fn is_external(link: &str) -> bool {
    let l = link.to_lowercase();
    l.starts_with("http://") || l.starts_with("https://")
}

pub fn contact_icon(link: &str) -> &'static str {
    let l = link.to_lowercase();
    if l.starts_with("mailto:") {
        return "mail";
    }
    if l.starts_with("tel:") {
        return "call";
    }
    if l.contains("t.me") || l.contains("telegram") {
        return "send";
    }
    if l.contains("youtube.com") || l.contains("youtu.be") {
        return "play_circle";
    }
    if l.contains("tangled.org") {
        return "hub";
    }
    if l.contains("github.com") || l.contains("codeberg.org") || l.contains("gitlab.com") {
        return "terminal";
    }
    "link"
}

pub fn device_icon(device: &str) -> &'static str {
    match device {
        "iphone" | "android" => "smartphone",
        "macos" | "windows" | "linux" | "desktop" => "computer",
        _ => "language",
    }
}

const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn date_parts(ts: i64) -> (i64, usize, i64) {
    let days = ts.div_euclid(86400);
    let (y, m, d) = civil_from_days(days);
    (y, (m - 1) as usize, d as i64)
}

pub fn fmt_date_unix(ts: i64) -> String {
    let (y, m, d) = date_parts(ts);
    let h = ts.rem_euclid(86400) / 3600;
    let mi = (ts.rem_euclid(3600)) / 60;
    format!("{d} {} {y} {h:02}:{mi:02}", MONTHS[m])
}

pub fn fmt_date_short_unix(ts: i64) -> String {
    let (_, m, d) = date_parts(ts);
    format!("{d} {}", MONTHS[m])
}

pub fn fmt_blog_date(date: &str) -> String {
    match date.trim().parse::<i64>() {
        Ok(ts) => fmt_date_unix(ts),
        Err(_) => date.to_string(),
    }
}

pub fn fmt_blog_date_short(date: &str) -> String {
    match date.trim().parse::<i64>() {
        Ok(ts) => fmt_date_short_unix(ts),
        Err(_) => date.to_string(),
    }
}
