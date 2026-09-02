//! Deterministic natural-language-to-intent normalization seam.
//!
//! This module deliberately does not pretend to be a general language model.
//! It extracts only explicit, auditable facts from text, reports missing
//! required fields, and compiles a confirmed `WorldIntent` into the one Rust
//! `WorldManifest` format consumed by the generator.

use serde::{Deserialize, Serialize};

use crate::worldgen::{
    SceneGraph, SceneSpec, WORLD_FORMAT, WORLD_VERSION, WorldManifest, WorldSpec,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerrainIntent {
    Plain,
    Coastal,
    River,
    Lake,
    Steppe,
    Mountain,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettlementIntent {
    City,
    Town,
    Pastoral,
    Wilderness,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaterIntent {
    None,
    River,
    Lake,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IntentScene {
    pub scene_id: String,
    pub width_m: u32,
    pub depth_m: u32,
    #[serde(default)]
    pub origin_x: i32,
    #[serde(default)]
    pub origin_z: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorldIntent {
    pub name: String,
    pub seed: u64,
    pub scene: IntentScene,
    pub terrain: TerrainIntent,
    pub settlement: SettlementIntent,
    pub water: WaterIntent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntentDraft {
    pub source_text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<WorldIntent>,
    #[serde(default)]
    pub missing_fields: Vec<String>,
    #[serde(default)]
    pub recognized: Vec<String>,
}

impl IntentDraft {
    pub fn is_ready(&self) -> bool {
        self.intent.is_some() && self.missing_fields.is_empty()
    }
}

fn contains_any(text: &str, values: &[&str]) -> bool {
    values.iter().any(|value| text.contains(value))
}

fn parse_seed(text: &str) -> Option<u64> {
    for marker in ["seed=", "seed：", "种子=", "种子："] {
        if let Some(rest) = text.split_once(marker).map(|(_, rest)| rest) {
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            if !digits.is_empty() {
                return digits.parse().ok();
            }
        }
    }
    None
}

fn parse_named_value(text: &str, markers: &[&str]) -> Option<String> {
    for marker in markers {
        if let Some(rest) = text.split_once(marker).map(|(_, rest)| rest.trim_start()) {
            let value: String = rest
                .chars()
                .take_while(|ch| !matches!(ch, '\n' | '\r' | '，' | ',' | '。' | ';' | '；'))
                .collect();
            let value = value.trim().trim_matches(['"', '“', '”', '\'']);
            if !value.is_empty() {
                return Some(value.to_owned());
            }
        }
    }
    None
}

fn parse_dimension_token(token: &str) -> Option<u32> {
    let digits: String = token.chars().filter(char::is_ascii_digit).collect();
    let value: u32 = digits.parse().ok()?;
    if token.contains("km") || token.contains("公里") || token.contains("千米") {
        Some(value.saturating_mul(1_000))
    } else if token.contains('m') || token.contains('米') {
        Some(value)
    } else {
        None
    }
}

fn parse_dimension_m(text: &str) -> Option<(u32, u32)> {
    let compact: String = text.chars().filter(|ch| !ch.is_whitespace()).collect();
    for (index, ch) in compact.char_indices() {
        if !matches!(ch, 'x' | 'X' | '×') {
            continue;
        }
        let left: String = compact[..index]
            .chars()
            .rev()
            .take_while(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '公' | '里' | '千' | '米'))
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        let right: String = compact[index + ch.len_utf8()..]
            .chars()
            .take_while(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '公' | '里' | '千' | '米'))
            .collect();
        if let (Some(width), Some(depth)) =
            (parse_dimension_token(&left), parse_dimension_token(&right))
        {
            return Some((width, depth));
        }
    }
    None
}

/// Extract auditable facts from free text. Anything not explicit is returned as
/// a missing field instead of silently invented.
pub fn draft_from_text(source_text: &str) -> IntentDraft {
    let text = source_text.to_lowercase();
    let mut missing: Vec<String> = Vec::new();
    let mut recognized: Vec<String> = Vec::new();
    let name = parse_named_value(
        source_text,
        &["名称：", "名称:", "世界名：", "世界名:", "name:"],
    );
    if name.is_some() {
        recognized.push("name".into());
    } else {
        missing.push("name".into());
    }
    let seed = parse_seed(&text);
    if seed.is_some() {
        recognized.push("seed".into());
    } else {
        missing.push("seed".into());
    }
    let size = parse_dimension_m(&text);
    if size.is_some() {
        recognized.push("scene.widthM/depthM".into());
    } else {
        missing.push("scene size, e.g. 2km x 1km".into());
    }

    let terrain = if contains_any(&text, &["海湾", "海岸", "coast", "bay"]) {
        TerrainIntent::Coastal
    } else if contains_any(&text, &["河", "river"]) {
        TerrainIntent::River
    } else if contains_any(&text, &["湖", "lake"]) {
        TerrainIntent::Lake
    } else if contains_any(&text, &["草原", "steppe", "牧场"]) {
        TerrainIntent::Steppe
    } else if contains_any(&text, &["山", "mountain", "峡谷", "valley"]) {
        TerrainIntent::Mountain
    } else {
        TerrainIntent::Plain
    };
    recognized.push("terrain".into());
    let water = match terrain {
        TerrainIntent::Coastal | TerrainIntent::River => WaterIntent::River,
        TerrainIntent::Lake => WaterIntent::Lake,
        _ => WaterIntent::None,
    };
    let settlement = if contains_any(&text, &["荒野", "无人区", "自然保护", "wilderness"])
    {
        SettlementIntent::Wilderness
    } else if contains_any(&text, &["牧场", "游牧", "pastoral"]) {
        SettlementIntent::Pastoral
    } else if contains_any(&text, &["城镇", "小镇", "town"]) {
        SettlementIntent::Town
    } else if contains_any(&text, &["城市", "城区", "city", "都市"]) {
        SettlementIntent::City
    } else {
        missing.push("settlement intent: city | town | pastoral | wilderness".into());
        SettlementIntent::Town
    };
    if !missing.iter().any(|item| item.starts_with("settlement")) {
        recognized.push("settlement".into());
    }

    let intent = if missing.is_empty() {
        let (width_m, depth_m) = size.expect("size required when draft ready");
        Some(WorldIntent {
            name: name.expect("name required when draft ready"),
            seed: seed.expect("seed required when draft ready"),
            scene: IntentScene {
                scene_id: "scene-0".into(),
                width_m,
                depth_m,
                origin_x: 0,
                origin_z: 0,
            },
            terrain,
            settlement,
            water,
        })
    } else {
        None
    };
    IntentDraft {
        source_text: source_text.into(),
        intent,
        missing_fields: missing,
        recognized,
    }
}

impl WorldIntent {
    pub fn compile_manifest(&self) -> WorldManifest {
        let (theme, natural_only, terrain_profile) = match (&self.terrain, &self.settlement) {
            (_, SettlementIntent::Wilderness) => ("temperate-plain", true, "grassland"),
            (TerrainIntent::Steppe, SettlementIntent::Pastoral) => {
                ("temperate-plain", false, "steppe")
            }
            (TerrainIntent::Coastal, _) => ("coastal-bay", false, "coastal"),
            (TerrainIntent::River, _) => ("river-delta", false, "plains"),
            (TerrainIntent::Lake, _) => ("temperate-plain", false, "plains"),
            (TerrainIntent::Mountain, _) => ("mountain-valley", false, "mountain"),
            (_, SettlementIntent::Town) => ("low-density-suburban", false, "plains"),
            _ => ("temperate-plain", false, "plains"),
        };
        let water_type = match self.water {
            WaterIntent::None => None,
            WaterIntent::River => Some("river"),
            WaterIntent::Lake => Some("lake"),
        };
        let mut style = serde_json::json!({
            "family": "procedural-modern-world",
            "terrain": "continuous-heightfield",
            "terrainProfile": terrain_profile,
            "generationIntent": {
                "terrain": self.terrain.clone(),
                "settlement": self.settlement.clone(),
                "water": self.water.clone(),
            },
            "landUseProfile": { "theme": theme },
            "naturalOnly": natural_only,
        });
        if let Some(water_type) = water_type {
            style["water"] = serde_json::json!({ "waterType": water_type, "levelPolicy": "horizontal-datum", "levelM": 0 });
        }
        WorldManifest {
            format: WORLD_FORMAT.into(),
            version: WORLD_VERSION,
            world: WorldSpec {
                name: self.name.clone(),
                seed: self.seed,
                render_mode: "2.5d".into(),
            },
            scenes: vec![SceneSpec {
                scene_id: self.scene.scene_id.clone(),
                width_m: self.scene.width_m,
                depth_m: self.scene.depth_m,
                origin_x: self.scene.origin_x,
                origin_z: self.scene.origin_z,
                seed_offset: 0,
            }],
            style,
            landmarks: Vec::new(),
            scene_graph: SceneGraph::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draft_reports_missing_critical_fields_without_inventing_them() {
        let draft = draft_from_text("生成一个沿海城市");
        assert!(draft.intent.is_none());
        assert!(draft.missing_fields.iter().any(|field| field == "name"));
        assert!(draft.missing_fields.iter().any(|field| field == "seed"));
    }

    #[test]
    fn draft_compiles_explicit_chinese_intent() {
        let draft = draft_from_text("名称：测试海湾，seed=42，2km x 1km 的沿海城市");
        let intent = draft.intent.expect("complete draft");
        let manifest = intent.compile_manifest();
        assert_eq!(manifest.world.name, "测试海湾");
        assert_eq!(manifest.scenes[0].width_m, 2_000);
        assert_eq!(manifest.style["water"]["waterType"], "river");
    }

    #[test]
    fn pastoral_steppe_is_not_compiled_as_pure_wilderness() {
        let intent = WorldIntent {
            name: "牧场".into(),
            seed: 7,
            scene: IntentScene {
                scene_id: "steppe".into(),
                width_m: 2_000,
                depth_m: 2_000,
                origin_x: 0,
                origin_z: 0,
            },
            terrain: TerrainIntent::Steppe,
            settlement: SettlementIntent::Pastoral,
            water: WaterIntent::None,
        };
        assert_eq!(intent.compile_manifest().style["naturalOnly"], false);
    }
}
