//! Builds `crates/core/data/foods.json`, the nutrition table the app ships with, from USDA FoodData
//! Central's SR Legacy download and `crates/core/data/foods-map.tsv` (ingredient key → USDA food,
//! reviewed by hand). Every number is USDA's: values per 100 g and its own household weights.
//!
//! ```sh
//! # https://fdc.nal.usda.gov/download-datasets → SR Legacy, JSON (public domain, CC0)
//! cargo run -p recipes-core --bin foods -- FoodData_Central_sr_legacy_food_json_2018-04.json
//! # add lookups the app made, once reviewed on the Ingredients page:
//! cargo run -p recipes-core --bin foods -- <sr-legacy.json> --merge "<data folder>/foods.json"
//! ```

use recipes_core::nutrition::{Food, UsdaFood, to_food};
use std::collections::{BTreeMap, HashMap};

#[derive(serde::Deserialize)]
struct Download {
    #[serde(rename = "SRLegacyFoods")]
    foods: Vec<UsdaFood>,
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let src = args.first().ok_or_else(|| anyhow::anyhow!("usage: foods <sr-legacy.json> [--merge <foods.json>]"))?;
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let sr: Download = serde_json::from_reader(std::io::BufReader::new(std::fs::File::open(src)?))?;
    let by_id: HashMap<u64, &UsdaFood> = sr.foods.iter().map(|f| (f.fdc_id, f)).collect();
    let mut foods: BTreeMap<String, Food> = BTreeMap::new();
    let mut missing = vec![];
    for line in std::fs::read_to_string(data.join("foods-map.tsv"))?.lines().skip(1) {
        let cols: Vec<&str> = line.split('\t').collect();
        let (Some(key), Some(id)) = (cols.first(), cols.get(1).map(|s| s.trim()).filter(|s| !s.is_empty())) else { continue };
        match id.parse().ok().and_then(|id: u64| by_id.get(&id)) {
            Some(f) => {
                foods.insert(key.to_string(), to_food(f, cols.get(3).map(|s| s.trim()).filter(|s| !s.is_empty())));
            }
            None => missing.push(format!("{key} ({id})")),
        }
    }
    // reviewed lookups from a data folder: kept unless the map has the key
    if let (Some("--merge"), Some(path)) = (args.get(1).map(String::as_str), args.get(2)) {
        let own: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)?;
        for (k, v) in own["foods"].as_object().into_iter().flatten() {
            if let Ok(mut f) = serde_json::from_value::<Food>(v.clone()) {
                f.auto = false;
                foods.entry(k.clone()).or_insert(f);
            }
        }
    }
    let mut out = String::from("{\"source\": \"U.S. Department of Agriculture, Agricultural Research Service. FoodData Central: SR Legacy, April 2018. fdc.nal.usda.gov\",\n\"foods\": {\n");
    let rows: Vec<String> = foods.iter().map(|(k, f)| format!("{}: {}", serde_json::to_string(k).unwrap(), serde_json::to_string(f).unwrap())).collect();
    out.push_str(&rows.join(",\n"));
    out.push_str("\n}}\n");
    std::fs::write(data.join("foods.json"), out)?;
    println!("{} foods → {}", foods.len(), data.join("foods.json").display());
    if !missing.is_empty() {
        eprintln!("not in SR Legacy: {}", missing.join(", "));
    }
    Ok(())
}
