//! Slug rules (SPEC): lowercase, ASCII-fold diacritics, drop apostrophes, `&`→`and`,
//! non-alphanumeric runs → single `-`, trim, ≤80 chars. Matches the Phase-2 manifest
//! slugifier so vault filenames are stable identities.

use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

pub fn slugify(input: &str) -> String {
    // NFKD decompose, drop combining marks (é → e), lowercase.
    let folded: String = input
        .nfkd()
        .filter(|c| !is_combining_mark(*c))
        .collect::<String>()
        .to_lowercase();

    // Drop apostrophes; expand `&`.
    let mut expanded = String::new();
    for c in folded.chars() {
        match c {
            '\'' | '\u{2019}' => {}
            '&' => expanded.push_str(" and "),
            other => expanded.push(other),
        }
    }

    // Non-alphanumeric runs → single `-`.
    let mut out = String::new();
    let mut prev_dash = false;
    for c in expanded.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }

    let out: String = out.trim_matches('-').chars().take(80).collect();
    out.trim_end_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_manifest_rules() {
        assert_eq!(slugify("Bagels"), "bagels");
        assert_eq!(slugify("Shepherd's Pie"), "shepherds-pie");
        assert_eq!(slugify("Mac & Cheese"), "mac-and-cheese");
        assert_eq!(slugify("Crème Brûlée"), "creme-brulee");
        assert_eq!(slugify("Sugar, Christmas cut outs"), "sugar-christmas-cut-outs");
    }
}
