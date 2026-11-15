//! Deterministic ingredient-line parser. Ported from
//! `handoff/src-reference/models.rs::parse_raw_ingredient_line` + `ingredient_unit`.
//! Turns "1 1/2 cups flour, sifted" into `{quantity, unit, name, preparation}`.
//! Ingredient lines are stored verbatim in the vault; this structure is derived at
//! render/scale time and never written to disk (per SPEC).

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedIngredient {
    pub quantity: Option<String>,
    pub unit: Option<String>,
    pub name: Option<String>,
    pub preparation: Option<String>,
}

pub fn parse_raw_ingredient_line(raw: &str) -> ParsedIngredient {
    let s = raw.trim();

    // Split off a short preparation note after the first comma (≤ 4 words).
    let (main, preparation) = {
        let mut main = s;
        let mut prep: Option<String> = None;
        if let Some(idx) = s.find(',') {
            let candidate = s[idx + 1..].trim();
            if !candidate.is_empty() && candidate.split_whitespace().count() <= 4 {
                main = &s[..idx];
                prep = Some(candidate.to_string());
            }
        }
        (main.trim(), prep)
    };

    let tokens: Vec<&str> = main.split_whitespace().collect();
    if tokens.is_empty() {
        return ParsedIngredient::default();
    }

    // Leading numeric quantity: whole, "N/D", or mixed "N N/D".
    let first_is_digit = tokens[0].starts_with(|c: char| c.is_ascii_digit());
    let is_frac = |t: &str| t.contains('/') && t.starts_with(|c: char| c.is_ascii_digit());

    let (qty, qty_end) = if !first_is_digit {
        (None, 0)
    } else if is_frac(tokens[0]) {
        (Some(tokens[0].to_string()), 1)
    } else if tokens.len() >= 2 && is_frac(tokens[1]) {
        (Some(format!("{} {}", tokens[0], tokens[1])), 2)
    } else {
        (Some(tokens[0].to_string()), 1)
    };

    if qty.is_none() {
        return ParsedIngredient {
            quantity: None,
            unit: None,
            name: Some(main.to_string()),
            preparation,
        };
    }

    let (unit, name_start) = if qty_end < tokens.len() {
        match ingredient_unit(tokens[qty_end]) {
            Some(u) => (Some(u), qty_end + 1),
            None => (None, qty_end),
        }
    } else {
        (None, qty_end)
    };

    let name = tokens[name_start..].join(" ");
    ParsedIngredient {
        quantity: qty,
        unit,
        name: Some(if name.is_empty() { main.to_string() } else { name }),
        preparation,
    }
}

/// Canonicalize a unit token ("cups" → "cup"). Returns `None` for non-unit words.
pub fn ingredient_unit(s: &str) -> Option<String> {
    let lower = s.trim_end_matches('.').to_lowercase();
    Some(
        match lower.as_str() {
            "cup" | "cups" | "c" => "cup",
            "tablespoon" | "tablespoons" | "tbsp" | "tbsps" | "tbs" => "tablespoon",
            "teaspoon" | "teaspoons" | "tsp" | "tsps" => "teaspoon",
            "ounce" | "ounces" | "oz" => "ounce",
            "pound" | "pounds" | "lb" | "lbs" => "pound",
            "gram" | "grams" | "g" => "gram",
            "kilogram" | "kilograms" | "kg" => "kilogram",
            "milliliter" | "milliliters" | "ml" => "milliliter",
            "liter" | "liters" => "liter",
            "pint" | "pints" | "pt" => "pint",
            "quart" | "quarts" | "qt" => "quart",
            "gallon" | "gallons" => "gallon",
            "clove" | "cloves" => "clove",
            "bunch" | "bunches" => "bunch",
            "can" | "cans" => "can",
            "package" | "packages" | "pkg" => "package",
            "slice" | "slices" => "slice",
            "piece" | "pieces" => "piece",
            "sprig" | "sprigs" => "sprig",
            "stick" | "sticks" => "stick",
            "stalk" | "stalks" => "stalk",
            "head" | "heads" => "head",
            "pinch" | "pinches" => "pinch",
            "dash" | "dashes" => "dash",
            "drop" | "drops" => "drop",
            _ => return None,
        }
        .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mixed_quantity_unit_name() {
        let p = parse_raw_ingredient_line("1 1/2 cups flour");
        assert_eq!(p.quantity.as_deref(), Some("1 1/2"));
        assert_eq!(p.unit.as_deref(), Some("cup"));
        assert_eq!(p.name.as_deref(), Some("flour"));
        assert_eq!(p.preparation, None);
    }

    #[test]
    fn parses_preparation_and_countable() {
        let p = parse_raw_ingredient_line("2 eggs, lightly beaten");
        assert_eq!(p.quantity.as_deref(), Some("2"));
        assert_eq!(p.unit, None); // eggs is the name, no measuring unit
        assert_eq!(p.name.as_deref(), Some("eggs"));
        assert_eq!(p.preparation.as_deref(), Some("lightly beaten"));
    }
}
