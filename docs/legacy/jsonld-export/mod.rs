//! The canonical recipe model: YAML frontmatter + a structured body. `Recipe` is the
//! file-content model (frontmatter + body); path-derived identity (slug, category) lives
//! in the vault layer, not here.

pub mod body;
pub mod duration;
pub mod frontmatter;
pub mod ingredient;
pub mod jsonld;

#[cfg(test)]
mod roundtrip_tests;

#[allow(unused_imports)]
pub use body::{Body, Instruction, Section};
pub use frontmatter::Frontmatter;

use crate::error::{AppError, Result};

#[derive(Debug, Clone, PartialEq)]
pub struct Recipe {
    pub frontmatter: Frontmatter,
    pub body: Body,
}

impl Recipe {
    /// Parse a recipe from its markdown file contents.
    pub fn from_markdown(text: &str) -> Result<Recipe> {
        let (yaml, body_text) = split_frontmatter(text)?;
        Ok(Recipe {
            frontmatter: frontmatter::parse(yaml)?,
            body: body::parse(body_text),
        })
    }

    /// Emit the canonical markdown form (deterministic — the "strict out" contract).
    pub fn to_markdown(&self) -> Result<String> {
        let fm = frontmatter::emit(&self.frontmatter)?;
        let body = body::emit(&self.body);
        Ok(format!("{fm}\n{body}"))
    }

    pub fn title(&self) -> &str {
        &self.frontmatter.title
    }
}

/// Split `---\n<yaml>\n---\n<body>` into (yaml, body).
fn split_frontmatter(text: &str) -> Result<(&str, &str)> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text); // tolerate BOM
    if !text.starts_with("---") {
        return Err(AppError::Parse("file has no YAML frontmatter".into()));
    }
    // Body starts after the closing `---` line.
    let after_open = text
        .find('\n')
        .map(|i| &text[i + 1..])
        .ok_or_else(|| AppError::Parse("truncated frontmatter".into()))?;
    // Find a line that is exactly `---`.
    let open_len = text.len() - after_open.len();
    let mut search_from = 0;
    loop {
        let rel = after_open[search_from..]
            .find("---")
            .ok_or_else(|| AppError::Parse("unterminated frontmatter".into()))?;
        let pos = search_from + rel;
        let at_line_start = pos == 0 || after_open.as_bytes()[pos - 1] == b'\n';
        let rest = &after_open[pos + 3..];
        let at_line_end = rest.is_empty() || rest.starts_with('\n') || rest.starts_with("\r\n");
        if at_line_start && at_line_end {
            let yaml = &text[open_len..open_len + pos];
            let body = rest.strip_prefix('\n').unwrap_or(rest);
            return Ok((yaml, body));
        }
        search_from = pos + 3;
    }
}
