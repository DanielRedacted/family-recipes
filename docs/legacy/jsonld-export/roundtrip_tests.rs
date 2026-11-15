//! Round-trip contract tests (SPEC): generated recipes survive parse∘emit, the whole real
//! vault parses and re-emits idempotently, and JSON-LD export is structurally valid.

use super::body::{Body, Instruction};
use super::{jsonld, Frontmatter, Recipe};
use proptest::prelude::*;
use std::path::{Path, PathBuf};

fn vault_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("cookbook")
}

fn md_files() -> Vec<PathBuf> {
    walkdir::WalkDir::new(vault_dir())
        .into_iter()
        .filter_map(Result::ok)
        .map(|e| e.into_path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("md"))
        .collect()
}

fn category_of(path: &Path) -> Option<String> {
    path.parent()
        .filter(|p| *p != vault_dir())
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .map(str::to_string)
}

#[test]
fn whole_vault_parses_and_round_trips_idempotently() {
    let files = md_files();
    assert!(files.len() >= 260, "expected the full vault, found {}", files.len());

    let mut failures = Vec::new();
    for path in &files {
        let text = std::fs::read_to_string(path).unwrap();
        let r1 = match Recipe::from_markdown(&text) {
            Ok(r) => r,
            Err(e) => {
                failures.push(format!("PARSE {}: {e}", path.display()));
                continue;
            }
        };
        let emitted = r1.to_markdown().unwrap();
        let r2 = Recipe::from_markdown(&emitted).unwrap();
        if r1 != r2 {
            failures.push(format!("MODEL DRIFT {}", path.display()));
        }
        // Emit must be a fixed point (byte-stable on re-emit).
        if r2.to_markdown().unwrap() != emitted {
            failures.push(format!("EMIT NOT STABLE {}", path.display()));
        }
    }
    assert!(failures.is_empty(), "{} failures:\n{}", failures.len(), failures.join("\n"));
    println!("round-tripped {} vault files", files.len());
}

#[test]
fn jsonld_export_is_structurally_valid() {
    for path in md_files() {
        let text = std::fs::read_to_string(&path).unwrap();
        let recipe = Recipe::from_markdown(&text).unwrap();
        let cat = category_of(&path);
        let ld = jsonld::to_jsonld(&recipe, cat.as_deref());

        assert_eq!(ld["@type"], "Recipe", "{}", path.display());
        assert!(ld["name"].as_str().is_some_and(|s| !s.is_empty()), "{}", path.display());
        if !recipe.body.ingredients.is_empty() {
            assert!(
                ld["recipeIngredient"].as_array().is_some_and(|a| !a.is_empty()),
                "{} has ingredients but no recipeIngredient",
                path.display()
            );
        }
        // keywords must not duplicate recipeCategory (Google rule).
        if let (Some(kw), Some(c)) = (ld["keywords"].as_str(), ld["recipeCategory"].as_str()) {
            for k in kw.split(", ") {
                assert!(!k.eq_ignore_ascii_case(c), "keyword duplicates category in {}", path.display());
            }
        }
    }
}

#[test]
fn hard_cases_bagels_and_sourdough() {
    // Bagels: component group labels are preserved as ingredients, excluded from JSON-LD.
    let bagels = std::fs::read_to_string(vault_dir().join("breads/bagels.md")).unwrap();
    let r = Recipe::from_markdown(&bagels).unwrap();
    assert!(r.body.ingredients.iter().any(|i| i == "**Sponge:**"), "group label kept");
    let ld = jsonld::to_jsonld(&r, Some("breads"));
    let ings = ld["recipeIngredient"].as_array().unwrap();
    assert!(
        ings.iter().all(|v| !v.as_str().unwrap().starts_with("**")),
        "bold group labels excluded from recipeIngredient"
    );
    println!(
        "\nbagels.md → JSON-LD:\n{}",
        serde_json::to_string_pretty(&ld).unwrap()
    );

    // Boston Cream Pie: standalone bold section labels (Cake/Custard/Glaze) become
    // HowToSection on export, with steps grouped and numbering restarting per section.
    let bcp = std::fs::read_to_string(vault_dir().join("cakes/boston-cream-pie.md")).unwrap();
    let r = Recipe::from_markdown(&bcp).unwrap();
    assert!(
        r.body.instructions.iter().any(|i| matches!(i, Instruction::Section(_))),
        "standalone bold section labels parsed"
    );
    let ld = jsonld::to_jsonld(&r, Some("cakes"));
    let has_section = ld["recipeInstructions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["@type"] == "HowToSection");
    assert!(has_section, "sections export as HowToSection");
}

// ---- generated round-trip -------------------------------------------------

fn safe() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9 ]{0,20}"
        .prop_map(|s| s.trim().to_string())
        .prop_filter("nonempty", |s| !s.is_empty())
}

fn instruction() -> impl Strategy<Value = Instruction> {
    prop_oneof![
        safe().prop_map(Instruction::Step),
        safe().prop_map(Instruction::Section),
    ]
}

fn recipe() -> impl Strategy<Value = Recipe> {
    (
        safe(),                                   // title
        prop::option::of(safe()),                 // yield
        prop::option::of(safe()),                 // source
        prop::collection::vec(safe(), 0..3),      // tags
        prop::collection::vec(safe(), 1..5),      // ingredients
        prop::collection::vec(instruction(), 0..5),
    )
        .prop_map(|(title, yield_, source, tags, ingredients, instructions)| Recipe {
            frontmatter: Frontmatter { title, yield_, source, tags, ..Default::default() },
            body: Body { lead: None, ingredients, instructions, extra_sections: vec![] },
        })
}

proptest! {
    #[test]
    fn parse_of_emit_is_identity(r in recipe()) {
        let s = r.to_markdown().unwrap();
        let back = Recipe::from_markdown(&s).unwrap();
        prop_assert_eq!(r, back);
    }
}
