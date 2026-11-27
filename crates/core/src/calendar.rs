//! The Mac's calendar, read-only, through EventKit: events are changed in Calendar.app, never here.
//! Off until switched on in Settings → Calendar (`settings.calendar`). Each request opens its own
//! event store, so it always sees what Calendar.app has now. The UI asks when a calendar page is
//! shown or comes back to the front, and every 15 minutes while it stays on screen.

use crate::auth::{Caller, Role, Who};
use crate::server::{AppState, err};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Extension, Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Event {
    pub title: String,
    /// Unix seconds
    pub start: i64,
    pub end: i64,
    pub all_day: bool,
    pub calendar: String,
    pub location: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct Calendar {
    pub id: String,
    pub title: String,
}

/// All-day events first, then by start time, then by title.
pub fn order(v: &mut [Event]) {
    v.sort_by(|a, b| b.all_day.cmp(&a.all_day).then(a.start.cmp(&b.start)).then_with(|| a.title.cmp(&b.title)));
}

/// The longest range one request may ask for (a month view plus its edges).
const MAX_RANGE: i64 = 62 * 86_400;

type Res = Result<Response, Response>;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/calendar", get(events_route)).route("/api/calendar/calendars", get(calendars_route))
}

#[derive(Deserialize)]
struct Range {
    from: i64,
    to: i64,
}

/// Events between two moments (the browser sends its own local midnights). Signed-in people only.
async fn events_route(State(s): State<Arc<AppState>>, Extension(Who(who)): Extension<Who>, Query(r): Query<Range>) -> Res {
    who.ok_or_else(|| err(StatusCode::UNAUTHORIZED, "sign in first"))?;
    if r.to <= r.from || r.to - r.from > MAX_RANGE {
        return Err(err(StatusCode::BAD_REQUEST, "ask for two months at most"));
    }
    let cfg = s.settings.read().unwrap().calendar.clone();
    if !cfg.enabled || cfg.calendars.is_empty() {
        return Ok(Json(Vec::<Event>::new()).into_response());
    }
    let mut v = tokio::task::spawn_blocking(move || mac::events(r.from, r.to, &cfg.calendars)).await.unwrap();
    order(&mut v);
    Ok(Json(v).into_response())
}

/// The Mac's calendars for Settings; asks macOS for access the first time.
async fn calendars_route(State(_): State<Arc<AppState>>, caller: Caller) -> Res {
    caller.need(Role::Admin)?;
    let (access, calendars) = tokio::task::spawn_blocking(|| {
        let mut a = mac::access();
        if a == "ask" {
            a = if mac::request() { "ok" } else { "denied" };
        }
        (a, if a == "ok" { mac::calendars() } else { vec![] })
    })
    .await
    .unwrap();
    Ok(Json(json!({ "access": access, "calendars": calendars })).into_response())
}

#[cfg(target_os = "macos")]
mod mac {
    use super::{Calendar, Event};
    use block2::RcBlock;
    use objc2::rc::autoreleasepool;
    use objc2::runtime::{Bool, NSObjectProtocol};
    use objc2::sel;
    use objc2_event_kit::{EKAuthorizationStatus, EKEntityType, EKEventStore};
    use objc2_foundation::{NSArray, NSDate, NSError};
    use std::sync::mpsc;
    use std::time::Duration;

    /// "ok", "ask" or "denied" (write-only access can't read events, so it counts as denied).
    pub fn access() -> &'static str {
        match unsafe { EKEventStore::authorizationStatusForEntityType(EKEntityType::Event) } {
            EKAuthorizationStatus::FullAccess => "ok",
            EKAuthorizationStatus::NotDetermined => "ask",
            _ => "denied",
        }
    }

    /// Show macOS's permission prompt and wait for the answer.
    pub fn request() -> bool {
        let (tx, rx) = mpsc::channel();
        let done = RcBlock::new(move |granted: Bool, _: *mut NSError| {
            let _ = tx.send(granted.as_bool());
        });
        let store = unsafe { EKEventStore::new() };
        unsafe {
            // macOS 14 replaced the call; macOS 12 and 13 only have the old one
            if store.respondsToSelector(sel!(requestFullAccessToEventsWithCompletion:)) {
                store.requestFullAccessToEventsWithCompletion(RcBlock::as_ptr(&done));
            } else {
                #[allow(deprecated)]
                store.requestAccessToEntityType_completion(EKEntityType::Event, RcBlock::as_ptr(&done));
            }
        }
        // the prompt waits for a person; give up after a while rather than hold the thread forever
        rx.recv_timeout(Duration::from_secs(300)).unwrap_or(false)
    }

    pub fn calendars() -> Vec<Calendar> {
        if access() != "ok" {
            return vec![];
        }
        autoreleasepool(|_| unsafe {
            let store = EKEventStore::new();
            let mut v: Vec<Calendar> = store
                .calendarsForEntityType(EKEntityType::Event)
                .iter()
                .map(|c| Calendar { id: c.calendarIdentifier().to_string(), title: c.title().to_string() })
                .collect();
            v.sort_by_key(|c| c.title.to_lowercase());
            v
        })
    }

    /// Only the ticked calendars: macOS grants access to all of them, Recipes reads just these.
    pub fn events(from: i64, to: i64, shown: &[String]) -> Vec<Event> {
        if access() != "ok" {
            return vec![];
        }
        autoreleasepool(|_| unsafe {
            let store = EKEventStore::new();
            let cals: Vec<_> = store.calendarsForEntityType(EKEntityType::Event).iter().filter(|c| shown.contains(&c.calendarIdentifier().to_string())).collect();
            if cals.is_empty() {
                return vec![];
            }
            let cals = NSArray::from_retained_slice(&cals);
            let start = NSDate::dateWithTimeIntervalSince1970(from as f64);
            let end = NSDate::dateWithTimeIntervalSince1970(to as f64);
            let pred = store.predicateForEventsWithStartDate_endDate_calendars(&start, &end, Some(&cals));
            store
                .eventsMatchingPredicate(&pred)
                .iter()
                .map(|e| Event {
                    title: e.title().to_string(),
                    start: e.startDate().timeIntervalSince1970() as i64,
                    end: e.endDate().timeIntervalSince1970() as i64,
                    all_day: e.isAllDay(),
                    calendar: e.calendar().map(|c| c.title().to_string()).unwrap_or_default(),
                    location: e.location().map(|l| l.to_string()).filter(|l| !l.trim().is_empty()),
                })
                .collect()
        })
    }
}

/// Other systems have no Calendar.app.
#[cfg(not(target_os = "macos"))]
mod mac {
    use super::{Calendar, Event};
    pub fn access() -> &'static str {
        "denied"
    }
    pub fn request() -> bool {
        false
    }
    pub fn calendars() -> Vec<Calendar> {
        vec![]
    }
    pub fn events(_: i64, _: i64, _shown: &[String]) -> Vec<Event> {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_day_first_then_by_time() {
        let ev = |t: &str, start, all_day| Event { title: t.into(), start, end: start + 3600, all_day, calendar: "Home".into(), location: None };
        let mut v = vec![ev("dinner", 200, false), ev("lunch", 100, false), ev("birthday", 300, true), ev("anniversary", 300, true)];
        order(&mut v);
        let titles: Vec<_> = v.iter().map(|e| e.title.as_str()).collect();
        assert_eq!(titles, ["anniversary", "birthday", "lunch", "dinner"]);
    }

    #[test]
    fn off_by_default_and_kept() {
        let s = crate::server::Settings::default();
        assert!(!s.calendar.enabled && s.calendar.calendars.is_empty(), "no calendar shows until one is ticked");
        let mut s = s;
        s.calendar = crate::server::CalendarSettings { enabled: true, calendars: vec!["family".into()] };
        let back: crate::server::Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert!(back.calendar.enabled);
        assert_eq!(back.calendar.calendars, ["family"]);
        // a settings file from the first version (which listed hidden calendars) shows nothing
        let old: crate::server::Settings = serde_json::from_str(r#"{"calendar":{"enabled":true,"hidden":[]}}"#).unwrap();
        assert!(old.calendar.calendars.is_empty());
    }
}
