use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;

type HmacSha256 = Hmac<Sha256>;

pub const COOKIE_NAME: &str = "acct";
pub const COOKIE_MAX_AGE: i64 = 2 * 365 * 24 * 60 * 60;

fn mac(secret: &[u8], id: &str) -> Vec<u8> {
    let mut m = HmacSha256::new_from_slice(secret).expect("hmac key");
    m.update(id.as_bytes());
    m.finalize().into_bytes().to_vec()
}

pub fn sign(secret: &[u8], id: &str) -> String {
    let payload = format!("{}.{}", id, crate::api::now_unix());
    format!(
        "{}.{}",
        payload,
        URL_SAFE_NO_PAD.encode(mac(secret, &payload))
    )
}

pub fn verify(secret: &[u8], cookie_value: &str) -> Option<String> {
    let (payload, sig) = cookie_value.rsplit_once('.')?;
    if payload.is_empty() {
        return None;
    }
    let got = URL_SAFE_NO_PAD.decode(sig).ok()?;
    let expected = mac(secret, payload);
    if !bool::from(expected.as_slice().ct_eq(got.as_slice())) {
        return None;
    }
    match payload.split_once('.') {
        Some((id, ts)) => {
            if id.is_empty() {
                return None;
            }
            let ts: i64 = ts.parse().ok()?;
            let now = crate::api::now_unix();
            if ts > now + 300 || now.saturating_sub(ts) > COOKIE_MAX_AGE {
                return None;
            }
            Some(id.to_string())
        }
        None => Some(payload.to_string()),
    }
}

pub fn new_account_id() -> String {
    let mut b = [0u8; 16];
    getrandom::getrandom(&mut b).expect("crypto rng unavailable");
    URL_SAFE_NO_PAD.encode(b)
}
