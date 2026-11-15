//! Recipe body parse/emit: lead prose, `## Ingredients`, `## Instructions`, and any other
//! `## section` (Notes, unknown) preserved verbatim. Ingredients/Instructions are the only
//! machine-parsed regions; everything else round-trips as raw text.

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Body {
    /// Text before the first `##` heading (lead image + prose), verbatim.
    pub lead: Option<String>,
    /// Bullet-line contents under `## Ingredients` (bold group labels included).
    pub ingredients: Vec<String>,
    pub instructions: Vec<Instruction>,
    /// Every other `## section` (Notes and unknown), preserved in order.
    pub extra_sections: Vec<Section>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instruction {
    Step(String),
    Section(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub heading: String,
    pub body: String,
}

fn is_heading(line: &str) -> Option<&str> {
    line.strip_prefix("## ").map(str::trim)
}

pub fn parse(text: &str) -> Body {
    let lines: Vec<&str> = text.lines().collect();
    // Lead = everything before the first heading.
    let first = lines.iter().position(|l| is_heading(l).is_some());
    let (lead_lines, rest) = match first {
        Some(i) => (&lines[..i], &lines[i..]),
        None => (&lines[..], &lines[0..0]),
    };
    let lead = {
        let s = lead_lines.join("\n");
        let t = s.trim();
        (!t.is_empty()).then(|| t.to_string())
    };

    let mut body = Body { lead, ..Default::default() };

    // Split `rest` into (heading, section-lines) chunks.
    let mut i = 0;
    while i < rest.len() {
        let heading = is_heading(rest[i]).unwrap_or_default().to_string();
        let mut j = i + 1;
        while j < rest.len() && is_heading(rest[j]).is_none() {
            j += 1;
        }
        let section_lines = &rest[i + 1..j];
        match heading.to_lowercase().as_str() {
            "ingredients" => body.ingredients.extend(parse_ingredients(section_lines)),
            "instructions" => body.instructions.extend(parse_instructions(section_lines)),
            _ => body.extra_sections.push(Section {
                heading,
                body: section_lines.join("\n").trim().to_string(),
            }),
        }
        i = j;
    }
    body
}

fn parse_ingredients(lines: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in lines {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if let Some(rest) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
            out.push(rest.to_string());
        } else if let Some(last) = out.last_mut() {
            // Continuation of a multi-line ingredient.
            last.push('\n');
            last.push_str(line);
        } else {
            out.push(t.to_string());
        }
    }
    out
}

fn parse_instructions(lines: &[&str]) -> Vec<Instruction> {
    let mut out: Vec<Instruction> = Vec::new();
    for line in lines {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        // Bold-only line → section label.
        if let Some(inner) = t.strip_prefix("**").and_then(|s| s.strip_suffix("**")) {
            if !inner.contains("**") {
                out.push(Instruction::Section(inner.to_string()));
                continue;
            }
        }
        // "N. text" → step.
        if let Some(rest) = strip_ordered_marker(t) {
            out.push(Instruction::Step(rest.to_string()));
        } else if let Some(Instruction::Step(prev)) = out.last_mut() {
            prev.push('\n');
            prev.push_str(line);
        } else {
            out.push(Instruction::Step(t.to_string()));
        }
    }
    out
}

/// Strip a leading `\d+.` / `\d+)` ordered-list marker, returning the text after it.
fn strip_ordered_marker(t: &str) -> Option<&str> {
    let digits: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    let after = &t[digits.len()..];
    let after = after.strip_prefix('.').or_else(|| after.strip_prefix(')'))?;
    Some(after.trim_start())
}

pub fn emit(body: &Body) -> String {
    let mut out = String::new();
    if let Some(lead) = &body.lead {
        out.push_str(lead.trim());
        out.push_str("\n\n");
    }
    if !body.ingredients.is_empty() {
        out.push_str("## Ingredients\n\n");
        for ing in &body.ingredients {
            out.push_str("- ");
            out.push_str(ing);
            out.push('\n');
        }
        out.push('\n');
    }
    if !body.instructions.is_empty() {
        out.push_str("## Instructions\n\n");
        let mut n = 0;
        for item in &body.instructions {
            match item {
                Instruction::Section(s) => {
                    out.push_str("**");
                    out.push_str(s);
                    out.push_str("**\n\n");
                    n = 0;
                }
                Instruction::Step(t) => {
                    n += 1;
                    out.push_str(&format!("{n}. {t}\n"));
                }
            }
        }
        out.push('\n');
    }
    for sec in &body.extra_sections {
        out.push_str("## ");
        out.push_str(&sec.heading);
        out.push_str("\n\n");
        out.push_str(sec.body.trim());
        out.push_str("\n\n");
    }
    let trimmed = out.trim_end();
    format!("{trimmed}\n")
}
