//! Who may do what.
//!
//! * Admin  — the desktop window: loopback listener + per-launch secret cookie.
//! * Writer — a LAN device that entered the PIN (signed session cookie).
//! * Reader — everyone else who can reach a listener.

use crate::server::{AppState, Listener};
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::extract::{FromRequestParts, Request, State};
use axum::http::{HeaderMap, Method, StatusCode, header, request::Parts};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use hmac::{Hmac, Mac};
use serde::Serialize;
use sha2::Sha256;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const ADMIN_COOKIE: &str = "recipes_admin";
pub const SESSION_COOKIE: &str = "__Host-recipes";
const SESSION_DAYS: u64 = 90;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Reader,
    Writer,
    Admin,
}

pub fn random_token() -> String {
    let bytes: [u8; 32] = rand::random();
    B64.encode(bytes)
}

pub fn hash_pin(pin: &str) -> String {
    let salt = SaltString::encode_b64(&rand::random::<[u8; 16]>()).unwrap();
    Argon2::default().hash_password(pin.as_bytes(), &salt).unwrap().to_string()
}

pub fn verify_pin(pin: &str, hash: &str) -> bool {
    PasswordHash::new(hash).is_ok_and(|h| Argon2::default().verify_password(pin.as_bytes(), &h).is_ok())
}

fn sign(key: &[u8], msg: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).unwrap();
    mac.update(msg.as_bytes());
    B64.encode(mac.finalize().into_bytes())
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

/// `<expiry>.<pin generation>.<signature>`
pub fn make_session(key: &[u8], pin_gen: u64) -> String {
    let body = format!("{}.{}", now() + SESSION_DAYS * 86400, pin_gen);
    format!("{body}.{}", sign(key, &body))
}

pub fn check_session(key: &[u8], pin_gen: u64, value: &str) -> bool {
    let mut parts = value.rsplitn(2, '.');
    let (Some(sig), Some(body)) = (parts.next(), parts.next()) else { return false };
    let expected = sign(key, body);
    if !constant_eq(sig.as_bytes(), expected.as_bytes()) {
        return false;
    }
    let mut f = body.split('.');
    let exp = f.next().and_then(|x| x.parse::<u64>().ok()).unwrap_or(0);
    let g = f.next().and_then(|x| x.parse::<u64>().ok());
    exp > now() && g == Some(pin_gen)
}

pub fn constant_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

pub fn cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|kv| kv.trim().split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v)
}

pub fn role_of(state: &AppState, listener: Listener, headers: &HeaderMap) -> Role {
    match listener {
        Listener::Loopback => {
            if cookie(headers, ADMIN_COOKIE).is_some_and(|c| constant_eq(c.as_bytes(), state.launch_token.as_bytes())) {
                return Role::Admin;
            }
        }
        Listener::Lan => {}
    }
    let s = state.settings.read().unwrap();
    if s.pin_hash.is_some()
        && cookie(headers, SESSION_COOKIE).is_some_and(|c| check_session(&state.cookie_key, s.pin_gen, c))
    {
        return Role::Writer;
    }
    Role::Reader
}

/// Extractor: the caller's role (computed by the guard middleware).
#[derive(Clone, Copy)]
pub struct Caller(pub Role);

impl<S: Send + Sync> FromRequestParts<S> for Caller {
    type Rejection = StatusCode;
    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        parts.extensions.get::<Caller>().copied().ok_or(StatusCode::INTERNAL_SERVER_ERROR)
    }
}

impl Caller {
    pub fn need(self, role: Role) -> Result<(), Response> {
        if self.0 >= role {
            Ok(())
        } else {
            let msg = if role == Role::Admin { "only the desktop app can do this" } else { "enter the PIN to make changes" };
            Err((StatusCode::FORBIDDEN, msg).into_response())
        }
    }
}

/// Host allow-list (blocks DNS rebinding), CSRF header + Origin check on mutations, role extension.
pub async fn guard(State(state): State<Arc<AppState>>, mut req: Request, next: Next) -> Response {
    let listener = req.extensions().get::<Listener>().copied().unwrap_or(Listener::Lan);
    // HTTP/2 carries the host in the :authority pseudo-header (the URI), HTTP/1.1 in Host
    let host = req
        .uri()
        .authority()
        .map(|a| a.as_str().to_string())
        .or_else(|| req.headers().get(header::HOST).and_then(|h| h.to_str().ok()).map(str::to_string))
        .unwrap_or_default()
        .to_lowercase();
    if !state.host_allowed(&host, listener) {
        return (StatusCode::MISDIRECTED_REQUEST, "unknown host").into_response();
    }
    let mutating = !matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS);
    if mutating {
        if req.headers().get("x-recipes").and_then(|v| v.to_str().ok()) != Some("1") {
            return (StatusCode::FORBIDDEN, "missing X-Recipes header").into_response();
        }
        if let Some(origin) = req.headers().get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
            let o = origin.split("://").nth(1).unwrap_or("").to_lowercase();
            if o != host {
                return (StatusCode::FORBIDDEN, "cross-origin request refused").into_response();
            }
        }
    }
    let role = role_of(&state, listener, req.headers());
    req.extensions_mut().insert(Caller(role));
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    h.insert("x-content-type-options", "nosniff".parse().unwrap());
    h.insert("referrer-policy", "same-origin".parse().unwrap());
    res
}

/// Failure-based limiter for PIN attempts.
#[derive(Default)]
pub struct PinLimiter {
    per_ip: Mutex<HashMap<IpAddr, (u32, Instant)>>,
    global: Mutex<Vec<Instant>>,
}

impl PinLimiter {
    /// Ok(()) if an attempt is allowed now, Err(seconds to wait) otherwise.
    pub fn check(&self, ip: IpAddr) -> Result<(), u64> {
        let now = Instant::now();
        let mut g = self.global.lock().unwrap();
        g.retain(|t| now.duration_since(*t) < Duration::from_secs(600));
        if g.len() >= 20 {
            return Err(600 - now.duration_since(g[0]).as_secs());
        }
        let m = self.per_ip.lock().unwrap();
        if let Some((fails, last)) = m.get(&ip) {
            if *fails >= 5 {
                let wait = 60u64 << (fails - 5).min(6);
                let since = now.duration_since(*last).as_secs();
                if since < wait {
                    return Err(wait - since);
                }
            }
        }
        Ok(())
    }
    pub fn fail(&self, ip: IpAddr) {
        let now = Instant::now();
        self.global.lock().unwrap().push(now);
        let mut m = self.per_ip.lock().unwrap();
        let e = m.entry(ip).or_insert((0, now));
        e.0 += 1;
        e.1 = now;
    }
    pub fn success(&self, ip: IpAddr) {
        self.per_ip.lock().unwrap().remove(&ip);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sessions_and_pins() {
        let key = b"k".repeat(32);
        let s = make_session(&key, 3);
        assert!(check_session(&key, 3, &s));
        assert!(!check_session(&key, 4, &s), "PIN change must sign everyone out");
        assert!(!check_session(&key, 3, &s.replace('.', ",")));
        let tampered = format!("9999999999.3.{}", s.rsplit('.').next().unwrap());
        assert!(!check_session(&key, 3, &tampered));
        let h = hash_pin("123456");
        assert!(verify_pin("123456", &h) && !verify_pin("123457", &h));
    }

    #[test]
    fn limiter_locks_after_five_failures() {
        let l = PinLimiter::default();
        let ip: IpAddr = "192.168.1.9".parse().unwrap();
        for _ in 0..5 {
            assert!(l.check(ip).is_ok());
            l.fail(ip);
        }
        assert!(l.check(ip).is_err());
        assert!(l.check("192.168.1.10".parse().unwrap()).is_ok());
    }
}
