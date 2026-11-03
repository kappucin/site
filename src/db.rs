use std::sync::Arc;

use redb::{Database, TableDefinition};
use serde::{Deserialize, Serialize};

pub type Db = Arc<Database>;

pub(crate) const COMMENTS: TableDefinition<&str, &str> = TableDefinition::new("comments");
pub(crate) const COMMENT_VOTES: TableDefinition<&str, &str> = TableDefinition::new("comment_votes");
pub(crate) const BLOG_POSTS: TableDefinition<&str, &str> = TableDefinition::new("blog_posts");
pub(crate) const CAPTCHA_CHALLENGES: TableDefinition<&str, &str> =
    TableDefinition::new("captcha_challenges");
pub(crate) const CAPTCHA_FAILS: TableDefinition<&str, &str> =
    TableDefinition::new("captcha_fails");

#[derive(Debug, thiserror::Error)]
pub enum StoreErr {
    #[error("not found")]
    NotFound,
    #[error("not owner")]
    NotOwner,
    #[error("db: {0}")]
    Db(String),
}

pub(crate) fn dbe<E: std::fmt::Display>(e: E) -> StoreErr {
    StoreErr::Db(e.to_string())
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct CommentRow {
    pub id: String,
    pub name: String,
    pub text: String,
    pub score: i64,
    pub is_admin: bool,
    pub device: String,
    pub author_account: String,
    pub created_at: i64,
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct BlogRow {
    pub id: String,
    pub title: String,
    pub text: String,
    pub date: String,
    pub created_at: i64,
}

pub(crate) fn vote_key(comment_id: &str, voter: &str) -> String {
    format!("{comment_id}\u{0}{voter}")
}

pub fn open(path: &str) -> Result<Db, Box<dyn std::error::Error + Send + Sync>> {
    let db = Database::create(path)?;
    let txn = db.begin_write()?;
    {
        let _ = txn.open_table(COMMENTS)?;
        let _ = txn.open_table(COMMENT_VOTES)?;
        let _ = txn.open_table(BLOG_POSTS)?;
        let _ = txn.open_table(CAPTCHA_CHALLENGES)?;
        let _ = txn.open_table(CAPTCHA_FAILS)?;
    }
    txn.commit()?;
    Ok(Arc::new(db))
}

pub async fn call<F, R>(db: &Db, f: F) -> Result<R, StoreErr>
where
    F: FnOnce(&Database) -> Result<R, StoreErr> + Send + 'static,
    R: Send + 'static,
{
    let db = db.clone();
    match tokio::task::spawn_blocking(move || f(&db)).await {
        Ok(res) => res,
        Err(_) => Err(StoreErr::Db("db worker panicked".into())),
    }
}


