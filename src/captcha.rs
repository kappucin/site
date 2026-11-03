use redb::{Database, ReadableTable};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::api::now_unix;
use crate::db::{dbe, StoreErr, CAPTCHA_CHALLENGES, CAPTCHA_FAILS};

const FAIL_WINDOW: i64 = 600;
const MAX_EXTRA_DIFFICULTY: u32 = 8;

#[derive(Serialize, Deserialize, Clone)]
pub struct Challenge {
    pub prefix: String,
    pub n: u32,
    pub issued_at: i64,
    pub used: bool,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct FailCounter {
    pub count: u32,
    pub window_start: i64,
}

fn hex_salt() -> String {
    let mut b = [0u8; 16];
    getrandom::getrandom(&mut b).expect("crypto rng unavailable");
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn leading_zero_bits(digest: &[u8]) -> u32 {
    let mut bits = 0;
    for b in digest {
        if *b == 0 {
            bits += 8;
        } else {
            bits += b.leading_zeros();
            break;
        }
    }
    bits
}

fn pow_hash(prefix: &str, nonce: &str) -> [u8; 32] {
    let input = format!("{prefix}{nonce}");
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hasher.finalize().into()
}

pub fn bump_fails(db: &Database, ip_hash: &str) {
    let now = now_unix();
    let _ = (|| -> Result<(), StoreErr> {
        let txn = db.begin_write().map_err(dbe)?;
        let mut counter: FailCounter = {
            let t = txn.open_table(CAPTCHA_FAILS).map_err(dbe)?;
            let raw: Option<String> =
                t.get(ip_hash).map_err(dbe)?.map(|g| g.value().to_string());
            match raw {
                Some(s) => serde_json::from_str(&s).unwrap_or_default(),
                None => FailCounter::default(),
            }
        };
        if now - counter.window_start > FAIL_WINDOW {
            counter = FailCounter::default();
        }
        counter.count += 1;
        if counter.window_start == 0 {
            counter.window_start = now;
        }
        let json = serde_json::to_string(&counter).map_err(dbe)?;
        txn.open_table(CAPTCHA_FAILS)
            .map_err(dbe)?
            .insert(ip_hash, json.as_str())
            .map_err(dbe)?;
        txn.commit().map_err(dbe)?;
        Ok(())
    })();
}

pub fn clear_fails(db: &Database, ip_hash: &str) {
    let _ = (|| -> Result<(), StoreErr> {
        let txn = db.begin_write().map_err(dbe)?;
        txn.open_table(CAPTCHA_FAILS)
            .map_err(dbe)?
            .remove(ip_hash)
            .map_err(dbe)?;
        txn.commit().map_err(dbe)?;
        Ok(())
    })();
}

pub fn current_difficulty(db: &Database, ip_hash: &str, base_n: u32) -> u32 {
    let now = now_unix();
    let raw: Option<String> = (|| {
        let txn = db.begin_read().ok()?;
        let t = txn.open_table(CAPTCHA_FAILS).ok()?;
        t.get(ip_hash).ok()?.map(|g| g.value().to_string())
    })();
    match raw.and_then(|s| serde_json::from_str::<FailCounter>(&s).ok()) {
        Some(c) if now - c.window_start <= FAIL_WINDOW => base_n + c.count.min(MAX_EXTRA_DIFFICULTY),
        _ => base_n,
    }
}

pub struct Issued {
    pub id: String,
    pub prefix: String,
    pub n: u32,
}

pub fn issue(
    db: &Database,
    account: &str,
    ip_hash: &str,
    base_n: u32,
    ttl: i64,
) -> Result<Issued, StoreErr> {
    let now = now_unix();
    let n = current_difficulty(db, ip_hash, base_n);
    let prefix = format!("{}:{}:{}:", hex_salt(), account, ip_hash);
    let id = crate::idgen::new_id();
    let challenge = Challenge {
        prefix: prefix.clone(),
        n,
        issued_at: now,
        used: false,
    };
    let json = serde_json::to_string(&challenge).map_err(dbe)?;

    let txn = db.begin_write().map_err(dbe)?;
    {
        let mut challenges = txn.open_table(CAPTCHA_CHALLENGES).map_err(dbe)?;
        let cutoff = now - ttl;
        let mut expired = Vec::new();
        for entry in challenges.iter().map_err(dbe)? {
            let (k, v) = entry.map_err(dbe)?;
            let c: Option<Challenge> = serde_json::from_str(v.value()).ok();
            if let Some(c) = c {
                if c.issued_at < cutoff {
                    expired.push(k.value().to_string());
                }
            }
        }
        for k in expired {
            challenges.remove(k.as_str()).map_err(dbe)?;
        }
        challenges.insert(id.as_str(), json.as_str()).map_err(dbe)?;
    }
    txn.commit().map_err(dbe)?;

    Ok(Issued { id, prefix, n })
}

pub fn verify(db: &Database, id: &str, nonce: &str, ttl: i64) -> Result<bool, StoreErr> {
    let now = now_unix();
    let txn = db.begin_write().map_err(dbe)?;
    let raw: Option<String> = {
        let t = txn.open_table(CAPTCHA_CHALLENGES).map_err(dbe)?;
        let x = t.get(id).map_err(dbe)?.map(|g| g.value().to_string());
        x
    };
    let mut c: Challenge = match raw {
        Some(s) => serde_json::from_str(&s).unwrap_or(Challenge {
            prefix: String::new(),
            n: 0,
            issued_at: 0,
            used: false,
        }),
        None => return Ok(false),
    };
    let elapsed = now - c.issued_at;
    let passed =
        !c.used && elapsed <= ttl && leading_zero_bits(&pow_hash(&c.prefix, nonce)) >= c.n;
    c.used = true;
    let json = serde_json::to_string(&c).map_err(dbe)?;
    txn.open_table(CAPTCHA_CHALLENGES)
        .map_err(dbe)?
        .insert(id, json.as_str())
        .map_err(dbe)?;
    txn.commit().map_err(dbe)?;
    Ok(passed)
}
