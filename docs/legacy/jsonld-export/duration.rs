//! ISO-8601 duration handling. Ported from
//! `handoff/src-reference/prompt.rs::parse_iso_duration_to_mins`, plus a human renderer.
//! Durations are stored verbatim in frontmatter (e.g. `PT25M`) and rendered for display.

/// Parse an ISO-8601 duration to minutes. Only hours/minutes of the time portion are
/// counted (days/weeks/years ignored — recipe times never need them).
pub fn parse_iso_duration_to_mins(s: &str) -> Option<i64> {
    if !s.starts_with('P') {
        return None;
    }
    let s = s.trim_start_matches('P');
    let s = if let Some(t) = s.find('T') { &s[t + 1..] } else { s };

    let mut mins = 0i64;
    let mut cur = String::new();
    for ch in s.chars() {
        if ch.is_ascii_digit() {
            cur.push(ch);
        } else {
            if let Ok(n) = cur.parse::<i64>() {
                match ch {
                    'H' => mins += n * 60,
                    'M' => mins += n,
                    _ => {}
                }
            }
            cur.clear();
        }
    }
    (mins > 0).then_some(mins)
}

/// Render minutes as a human string: 25 → "25 min", 90 → "1 hr 30 min", 120 → "2 hr".
pub fn human_minutes(mins: i64) -> String {
    let (h, m) = (mins / 60, mins % 60);
    match (h, m) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} hr"),
        (h, m) => format!("{h} hr {m} min"),
    }
}

/// Render an ISO-8601 duration string for display, falling back to the raw string.
pub fn human_duration(iso: &str) -> String {
    parse_iso_duration_to_mins(iso)
        .map(human_minutes)
        .unwrap_or_else(|| iso.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations() {
        assert_eq!(parse_iso_duration_to_mins("PT25M"), Some(25));
        assert_eq!(parse_iso_duration_to_mins("PT1H30M"), Some(90));
        assert_eq!(parse_iso_duration_to_mins("PT2H"), Some(120));
        assert_eq!(parse_iso_duration_to_mins("garbage"), None);
        assert_eq!(human_duration("PT1H30M"), "1 hr 30 min");
        assert_eq!(human_duration("PT25M"), "25 min");
    }
}
