//! Quantities as exact `(numerator, denominator)` fractions — never floats — plus
//! the serving scaler and its unit-aware kitchen rounding.
//!
//! Ported from `handoff/src-reference/prompt.rs` (parse + gcd) and
//! `.claude/skills/fractions.md` (unicode display). The scaler and rounding are new.

/// A reduced fraction with a positive denominator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fraction {
    pub num: i64,
    pub den: i64,
}

impl Fraction {
    /// Construct and reduce. Returns `None` for a zero denominator.
    pub fn new(n: i64, d: i64) -> Option<Self> {
        if d == 0 {
            return None;
        }
        let (n, d) = if d < 0 { (-n, -d) } else { (n, d) };
        let g = gcd(n.abs(), d).max(1);
        Some(Self { num: n / g, den: d / g })
    }

    pub fn whole(n: i64) -> Self {
        Self { num: n, den: 1 }
    }

    pub fn value(self) -> f64 {
        self.num as f64 / self.den as f64
    }

    /// Multiply two fractions (used to apply a servings ratio).
    pub fn mul(self, other: Fraction) -> Fraction {
        // Safe for cookbook-scale magnitudes; reduces immediately.
        Fraction::new(self.num * other.num, self.den * other.den).unwrap()
    }

    /// Scale by `new_servings / default_servings`.
    pub fn scale(self, new_servings: i64, default_servings: i64) -> Option<Fraction> {
        let ratio = Fraction::new(new_servings, default_servings)?;
        Some(self.mul(ratio))
    }

    /// Snap to the nearest sensible kitchen measurement.
    ///
    /// - **Countable** ingredients (no measuring unit, or an item unit like clove/can)
    ///   round to the nearest whole number — never "1⅓ eggs".
    /// - **Volume/weight** units snap the fractional part to the nearest common kitchen
    ///   fraction {1/8, 1/4, 1/3, 1/2, 2/3, 3/4}; nothing finer than 1/8, and a nonzero
    ///   amount never floors to 0 (minimum 1/8).
    ///
    /// ponytail: "countable = no unit or an item unit" is a heuristic; the P4 UI can
    /// refine per-ingredient. The floor and the candidate set are the tuning knobs.
    pub fn round_kitchen(self, unit: Option<&str>) -> Fraction {
        if self.num == 0 {
            return Fraction::whole(0);
        }
        if is_countable(unit) {
            let rounded = self.value().round() as i64;
            return Fraction::whole(rounded.max(1)); // a nonzero countable is at least 1
        }
        // Volume/weight: snap fractional part to a common kitchen fraction.
        let v = self.value();
        let whole = v.floor() as i64;
        let frac = v - whole as f64;
        const CANDS: [(i64, i64); 8] = [
            (0, 1), (1, 8), (1, 4), (1, 3), (1, 2), (2, 3), (3, 4), (1, 1),
        ];
        let (bn, bd) = CANDS
            .iter()
            .min_by(|a, b| {
                let da = (frac - a.0 as f64 / a.1 as f64).abs();
                let db = (frac - b.0 as f64 / b.1 as f64).abs();
                da.partial_cmp(&db).unwrap()
            })
            .copied()
            .unwrap();
        let result = Fraction::new(whole * bd + bn, bd).unwrap();
        if result.num == 0 {
            Fraction::new(1, 8).unwrap() // floor a nonzero amount at 1/8
        } else {
            result
        }
    }

    /// Display as a Unicode fraction: "1½", "¾", "2", "5⅓".
    pub fn display(self) -> String {
        let whole = self.num / self.den;
        let rem = self.num.abs() % self.den;
        if rem == 0 {
            return whole.to_string();
        }
        let g = gcd(rem, self.den).max(1);
        let glyph = match (rem / g, self.den / g) {
            (1, 2) => "½",
            (1, 3) => "⅓",
            (2, 3) => "⅔",
            (1, 4) => "¼",
            (3, 4) => "¾",
            (1, 8) => "⅛",
            (3, 8) => "⅜",
            (5, 8) => "⅝",
            (7, 8) => "⅞",
            (n, d) => {
                return if whole == 0 {
                    format!("{n}/{d}")
                } else {
                    format!("{whole} {n}/{d}")
                }
            }
        };
        if whole == 0 {
            glyph.to_string()
        } else {
            format!("{whole}{glyph}")
        }
    }
}

/// Units that count discrete items (round to whole on scaling).
fn is_countable(unit: Option<&str>) -> bool {
    match unit {
        None => true, // "3 eggs", "2 bananas" — no measuring unit
        Some(u) => matches!(
            u.trim().to_lowercase().as_str(),
            "clove" | "cloves" | "can" | "cans" | "slice" | "slices" | "piece" | "pieces"
                | "stick" | "sticks" | "head" | "heads" | "package" | "packages" | "pkg"
                | "bunch" | "bunches" | "sprig" | "sprigs" | "stalk" | "stalks" | "egg" | "eggs"
        ),
    }
}

pub fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

/// Unicode fraction glyphs → ASCII (`½` → `1/2`). Ported verbatim; no inverse
/// (the inverse is `Fraction::display`).
pub fn normalise_fractions(text: &str) -> String {
    text.replace('½', "1/2")
        .replace('⅓', "1/3")
        .replace('⅔', "2/3")
        .replace('¼', "1/4")
        .replace('¾', "3/4")
        .replace('⅕', "1/5")
        .replace('⅖', "2/5")
        .replace('⅗', "3/5")
        .replace('⅘', "4/5")
        .replace('⅙', "1/6")
        .replace('⅚', "5/6")
        .replace('⅛', "1/8")
        .replace('⅜', "3/8")
        .replace('⅝', "5/8")
        .replace('⅞', "7/8")
}

/// Map unicode fraction glyphs to space-padded ASCII so a glyph glued to a whole number
/// ("1½") becomes a mixed number ("1 1/2") rather than "11/2".
fn glyphs_to_spaced_ascii(s: &str) -> String {
    let spaced = s
        .replace('½', " 1/2 ")
        .replace('⅓', " 1/3 ")
        .replace('⅔', " 2/3 ")
        .replace('¼', " 1/4 ")
        .replace('¾', " 3/4 ")
        .replace('⅕', " 1/5 ")
        .replace('⅖', " 2/5 ")
        .replace('⅗', " 3/5 ")
        .replace('⅘', " 4/5 ")
        .replace('⅙', " 1/6 ")
        .replace('⅚', " 5/6 ")
        .replace('⅛', " 1/8 ")
        .replace('⅜', " 3/8 ")
        .replace('⅝', " 5/8 ")
        .replace('⅞', " 7/8 ");
    spaced.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Parse "1/2", "3/4", "2", "1 1/2", "1½" → `Fraction`. Ported from prompt.rs.
pub fn parse_fraction_str(input: &str) -> Option<Fraction> {
    let normalised;
    let s = if input.chars().any(|c| "½⅓⅔¼¾⅕⅖⅗⅘⅙⅚⅛⅜⅝⅞".contains(c)) {
        normalised = glyphs_to_spaced_ascii(input.trim());
        normalised.as_str()
    } else {
        input.trim()
    };
    if s.is_empty() {
        return None;
    }

    // Mixed number "1 1/2".
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() == 2 {
        if let (Ok(whole), Some(frac)) = (parts[0].parse::<i64>(), parse_fraction_str(parts[1])) {
            return Fraction::new(whole * frac.den + frac.num, frac.den);
        }
    }

    // Simple "1/2".
    if let Some(slash) = s.find('/') {
        let n: i64 = s[..slash].trim().parse().ok()?;
        let d: i64 = s[slash + 1..].trim().parse().ok()?;
        return Fraction::new(n, d);
    }

    // Whole number.
    if let Ok(n) = s.parse::<i64>() {
        return Some(Fraction::whole(n));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(n: i64, d: i64) -> Fraction {
        Fraction::new(n, d).unwrap()
    }

    #[test]
    fn parse_and_display() {
        assert_eq!(parse_fraction_str("1/2"), Some(f(1, 2)));
        assert_eq!(parse_fraction_str("1 1/2"), Some(f(3, 2)));
        assert_eq!(parse_fraction_str("1½"), Some(f(3, 2)));
        assert_eq!(parse_fraction_str("2"), Some(f(2, 1)));
        assert_eq!(parse_fraction_str("garbage"), None);
        assert_eq!(f(1, 2).display(), "½");
        assert_eq!(f(3, 2).display(), "1½");
        assert_eq!(f(4, 1).display(), "4");
        assert_eq!(f(16, 3).display(), "5⅓"); // scaled result renders with the ⅓ glyph
    }

    #[test]
    fn scaling_reduces_exactly() {
        // 1/3 cup doubled = 2/3 (no float drift).
        assert_eq!(f(1, 3).scale(2, 1), Some(f(2, 3)));
        // 1/3 × 3 = exactly 1.
        assert_eq!(f(1, 3).scale(3, 1), Some(f(1, 1)));
    }

    #[test]
    fn kitchen_rounding_rule() {
        // Countable: eggs never fractional.
        assert_eq!(f(3, 2).round_kitchen(None), Fraction::whole(2)); // 1½ eggs → 2
        assert_eq!(f(1, 3).round_kitchen(None), Fraction::whole(1)); // ⅓ egg → 1 (nonzero floor)
        assert_eq!(f(2, 1).round_kitchen(Some("clove")), Fraction::whole(2));
        // Volume/weight: snap to common fractions.
        assert_eq!(f(2, 3).round_kitchen(Some("cup")), f(2, 3)); // already common
        assert_eq!(f(3, 4).round_kitchen(Some("teaspoon")), f(3, 4));
        assert_eq!(f(1, 16).round_kitchen(Some("teaspoon")), f(1, 8)); // finer than 1/8 → floor
        assert_eq!(f(7, 16).round_kitchen(Some("cup")), f(1, 2)); // 0.4375 → nearest ½
        assert_eq!(f(5, 2).round_kitchen(Some("cup")), f(5, 2)); // 2½ stays
        // End-to-end: scale then round.
        let scaled = f(1, 4).scale(3, 1).unwrap().round_kitchen(Some("teaspoon"));
        assert_eq!(scaled, f(3, 4)); // ¼ tsp × 3 = ¾ tsp
    }
}
