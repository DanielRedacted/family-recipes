//! Who may do what.
//!
//! * Admin  — the desktop window: loopback listener + per-launch secret cookie.
//! * Writer — a person signed in on a LAN device with the family PIN (signed session cookie).
//! * Reader — everyone else who can reach a listener (signed in or not while no PIN is set).
//!
//! Who is asking (`Who`) is separate from the role: the desktop window is the built-in "Admin",
//! a signed-in device is its person (see users.rs), anyone else is nobody.

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
/// The same session on the plain-HTTP listener (a `__Host-` cookie needs HTTPS).
pub const HTTP_SESSION_COOKIE: &str = "recipes_session";
/// The longest a browser keeps a cookie (400 days); signing in again renews it.
pub const SESSION_DAYS: u64 = 400;

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

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

/// `<expiry>.<pin generation>.<user id>.<signature>` (user ids never contain a dot)
pub fn make_session(key: &[u8], pin_gen: u64, user: &str) -> String {
    let body = format!("{}.{}.{}", now() + SESSION_DAYS * 86400, pin_gen, user);
    format!("{body}.{}", sign(key, &body))
}

/// The user id of a valid, unexpired session from the current PIN generation.
pub fn check_session(key: &[u8], pin_gen: u64, value: &str) -> Option<String> {
    let mut parts = value.rsplitn(2, '.');
    let (Some(sig), Some(body)) = (parts.next(), parts.next()) else { return None };
    let expected = sign(key, body);
    if !constant_eq(sig.as_bytes(), expected.as_bytes()) {
        return None;
    }
    let mut f = body.split('.');
    let exp = f.next().and_then(|x| x.parse::<u64>().ok()).unwrap_or(0);
    let g = f.next().and_then(|x| x.parse::<u64>().ok());
    let user = f.next().filter(|u| !u.is_empty())?;
    (exp > now() && g == Some(pin_gen) && f.next().is_none()).then(|| user.to_string())
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

/// The desktop window's writes and notes are recorded under this built-in person.
pub const ADMIN_ID: &str = "admin";

/// A person: someone signed in on a device, or the desktop window ("Admin").
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Actor {
    pub id: String,
    pub name: String,
    /// for recipe credits; empty for "Admin"
    pub full_name: String,
}

/// Extension: who is asking (computed by the guard middleware); None before signing in.
#[derive(Clone)]
pub struct Who(pub Option<Actor>);

pub fn identify(state: &AppState, listener: Listener, headers: &HeaderMap) -> (Role, Option<Actor>) {
    if listener == Listener::Loopback && cookie(headers, ADMIN_COOKIE).is_some_and(|c| constant_eq(c.as_bytes(), state.launch_token.as_bytes())) {
        // the person chosen as "Editing as" on the Mac (Settings → People), else the built-in Admin
        let chosen = state.settings.read().unwrap().editing_as.clone();
        let actor = chosen.and_then(|id| crate::users::actor_of(state, &id)).unwrap_or_else(|| Actor { id: ADMIN_ID.into(), name: "Admin".into(), full_name: String::new() });
        return (Role::Admin, Some(actor));
    }
    let (pin_set, pin_gen) = {
        let s = state.settings.read().unwrap();
        (s.pin_hash.is_some(), s.pin_gen)
    };
    let user = [SESSION_COOKIE, HTTP_SESSION_COOKIE].iter().find_map(|n| cookie(headers, n).and_then(|c| check_session(&state.cookie_key, pin_gen, c)));
    // a removed person's cookie still verifies, but names nobody
    match user.and_then(|id| crate::users::actor_of(state, &id)) {
        Some(a) => (if pin_set { Role::Writer } else { Role::Reader }, Some(a)),
        None => (Role::Reader, None),
    }
}

pub fn role_of(state: &AppState, listener: Listener, headers: &HeaderMap) -> Role {
    identify(state, listener, headers).0
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
            let msg = if role == Role::Admin { "only the desktop app can do this" } else { "editing needs the family PIN: set one in the Recipes app on the Mac" };
            Err((StatusCode::FORBIDDEN, msg).into_response())
        }
    }
}

/// Host allow-list (blocks DNS rebinding), CSRF header + Origin check on mutations, role and person
/// extensions, and the record of every change a person made (users.rs).
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
    let (role, actor) = identify(&state, listener, req.headers());
    let write = mutating.then(|| (req.method().to_string(), req.uri().path().to_string(), crate::server::book_param(req.uri()).map(str::to_string)));
    req.extensions_mut().insert(Caller(role));
    req.extensions_mut().insert(Who(actor.clone()));
    let mut res = next.run(req).await;
    if let (Some((method, path, book)), Some(a)) = (write, &actor) {
        if res.status().is_success() {
            crate::users::log_write(&state, a, &method, &path, book.as_deref());
        }
    }
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
        let s = make_session(&key, 3, "ab12");
        assert_eq!(check_session(&key, 3, &s).as_deref(), Some("ab12"));
        assert_eq!(check_session(&key, 4, &s), None, "PIN change must sign everyone out");
        assert_eq!(check_session(&key, 3, &s.replace('.', ",")), None);
        let tampered = format!("9999999999.3.ab12.{}", s.rsplit('.').next().unwrap());
        assert_eq!(check_session(&key, 3, &tampered), None);
        // a cookie from before people had names (`<expiry>.<pin generation>.<signature>`)
        let body = format!("{}.3", now() + 60);
        assert_eq!(check_session(&key, 3, &format!("{body}.{}", sign(&key, &body))), None);
        let h = hash_pin("123456");
        assert!(verify_pin("123456", &h) && !verify_pin("123457", &h));
    }

    #[tokio::test]
    async fn signed_in_person_edits_with_the_pin() {
        let tmp = std::env::temp_dir().join(format!("recipes-auth-{}", std::process::id()));
        let cfg = crate::server::Config { vault: tmp.join("v"), data_dir: tmp.join("d"), shell: None, pdf_text: None };
        let state = crate::server::init(cfg).unwrap();
        let gen_ = state.settings.read().unwrap().pin_gen;
        let ann = crate::users::sign_in(&state, "Ann", "Ann Romo", "Safari", "192.168.1.9").unwrap();
        let session = make_session(&state.cookie_key, gen_, &ann.id);
        let with = |c: String| {
            let mut h = HeaderMap::new();
            h.insert(header::COOKIE, c.parse().unwrap());
            h
        };
        // no PIN set: signed in, but read-only
        let (role, who) = identify(&state, Listener::Lan, &with(format!("{SESSION_COOKIE}={session}")));
        assert_eq!((role, who.map(|a| a.name)), (Role::Reader, Some("Ann".to_string())));
        state.settings.write().unwrap().pin_hash = Some(hash_pin("1234"));
        assert_eq!(role_of(&state, Listener::LanHttp, &with(format!("{HTTP_SESSION_COOKIE}={session}"))), Role::Writer);
        assert_eq!(role_of(&state, Listener::Lan, &with(format!("{SESSION_COOKIE}={session}"))), Role::Writer);
        assert_eq!(role_of(&state, Listener::LanHttp, &with(format!("{HTTP_SESSION_COOKIE}=forged"))), Role::Reader);
        assert_eq!(role_of(&state, Listener::LanHttp, &HeaderMap::new()), Role::Reader);
        // removing the person signs their devices out
        crate::users::remove(&state, &ann.id).unwrap();
        assert_eq!(identify(&state, Listener::Lan, &with(format!("{SESSION_COOKIE}={session}"))), (Role::Reader, None));

        // the desktop window edits as the chosen person, as Admin while nobody (or a removed person) is chosen
        let ben = crate::users::sign_in(&state, "Ben", "Ben Romo", "Safari", "192.168.1.10").unwrap();
        let admin = with(format!("{ADMIN_COOKIE}={}", state.launch_token));
        assert_eq!(identify(&state, Listener::Loopback, &admin).1.map(|a| a.id).as_deref(), Some(ADMIN_ID));
        state.settings.write().unwrap().editing_as = Some(ben.id.clone());
        let (role, who) = identify(&state, Listener::Loopback, &admin);
        assert_eq!((role, who.map(|a| a.full_name)), (Role::Admin, Some("Ben Romo".to_string())));
        crate::users::remove(&state, &ben.id).unwrap();
        assert_eq!(identify(&state, Listener::Loopback, &admin).1.map(|a| a.id).as_deref(), Some(ADMIN_ID));
        let _ = std::fs::remove_dir_all(&tmp);
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
