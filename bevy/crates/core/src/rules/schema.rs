//! Strongly-typed object descriptor schema (TOML/JSON 鈫?Rust).
//!
//! Design doc: docs/rules-engine-mvp.zh-CN.md 搂3.
//! Every object in the world is described declaratively; the placement
//! engine validates candidates against these specs instead of bespoke
//! per-kind logic.

use serde::{Deserialize, Serialize};

/// High-level entity kinds the placement engine reasons about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Road,
    Railway,
    Building,
    Storefront,
    Tree,
    Rock,
    Water,
    Bridge,
    Park,
    Sidewalk,
    Lamp,
    Bench,
    BusStop,
    FoodStall,
    Other,
}

impl ItemKind {
    /// True if this kind occupies the "hard" collision layer (roads/buildings)
    /// vs the soft layer (vegetation/decoration). Mirrors `OccLayer`.
    pub fn is_hard(self) -> bool {
        matches!(
            self,
            ItemKind::Road
                | ItemKind::Railway
                | ItemKind::Building
                | ItemKind::Storefront
                | ItemKind::Bridge
                | ItemKind::Park
                | ItemKind::Sidewalk
        )
    }
}

impl Default for ItemKind {
    fn default() -> Self {
        ItemKind::Other
    }
}

/// Pivot of the model origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Pivot {
    #[default]
    Center,
    Bottom,
}

/// Allowed rotation behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RotationMode {
    #[default]
    Any,
    None,
    AlignToTarget,
}

/// Biome membership (subset 鈥?extend as the generator grows).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Biome {
    Grassland,
    Forest,
    Desert,
    Tundra,
    Urban,
    UrbanGreen,
    Water,
}

/// Hazards that forbid placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HazardKind {
    Cliff,
    Floodplain,
    Landslide,
}

/// Anchor semantics (entrance / service access).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AnchorKind {
    #[default]
    PublicAccess,
    Maintenance,
}

/// Generation phase: smaller = generated earlier. Matches the plan 搂2.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PlacementPhase {
    Terrain = 0,
    WaterAndHazards = 1,
    Railway = 2,
    #[default]
    Road = 3,
    Lot = 4,
    Building = 5,
    Functional = 6,
    Vegetation = 7,
    Decoration = 8,
}

/// Fallback strategy when no candidate passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Fallback {
    #[default]
    Skip,
    Move,
    Shrink,
}

/// Footprint + height + clearance of an object.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeometrySpec {
    /// [width, depth] in metres.
    pub footprint: [f32; 2],
    /// 3D collision height in metres.
    pub height: f32,
    /// Safety / maintenance margin added around the footprint for
    /// collision tests (metres).
    #[serde(default = "default_clearance")]
    pub clearance: f32,
    #[serde(default)]
    pub pivot: Pivot,
    #[serde(default)]
    pub rotations: RotationMode,
}

fn default_clearance() -> f32 {
    0.5
}

/// Environment constraints (ground, water, slope, biome).
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentSpec {
    /// Bottom must touch the ground (4-corner height test).
    #[serde(default)]
    pub on_ground: bool,
    /// Footprint must not enter water / floodplain.
    #[serde(default)]
    pub not_in_water: bool,
    /// Max slope in metres per 100m of run (e.g. 12 = 12%).
    #[serde(default)]
    pub max_slope: f32,
    #[serde(default)]
    pub allowed_biomes: Vec<Biome>,
    #[serde(default)]
    pub forbidden_hazards: Vec<HazardKind>,
}

/// A "must keep away from kind X by distance D" relation.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationAvoid {
    pub kind: ItemKind,
    #[serde(default)]
    pub distance: f32,
}

/// A "must / like to be near kind X within distance D" relation.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationNear {
    pub kind: ItemKind,
    pub distance: f32,
    #[serde(default)]
    pub weight: f32,
}

/// Relations to other objects (hard avoid/require + soft prefer).
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct RelationSpec {
    #[serde(default)]
    pub avoid: Vec<RelationAvoid>,
    #[serde(default)]
    pub require: Vec<RelationNear>,
    #[serde(default)]
    pub prefer: Vec<RelationNear>,
}

/// Anchor: an entrance / service point on a side of the footprint.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnchorSpec {
    pub id: String,
    #[serde(default)]
    pub side: String,
    #[serde(default)]
    pub kind: AnchorKind,
    /// Clearance radius around the anchor (front-of-door must stay empty).
    #[serde(default)]
    pub clear_radius: f32,
    /// Entity kind/tag the anchor must face (e.g. "road").
    #[serde(default)]
    pub must_face: Option<String>,
}

/// Placement strategy.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlacementSpec {
    pub phase: PlacementPhase,
    #[serde(default = "default_priority")]
    pub priority: u32,
    #[serde(default = "default_attempts")]
    pub attempts: u32,
    #[serde(default)]
    pub allow_rotate: bool,
    #[serde(default)]
    pub allow_scale: bool,
    #[serde(default)]
    pub fallback: Fallback,
}

fn default_priority() -> u32 {
    50
}
fn default_attempts() -> u32 {
    12
}

/// The top-level descriptor of one object type.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectDescriptor {
    pub id: String,
    pub kind: ItemKind,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Additional entity kind strings this descriptor governs (aliases). Audit
    /// / placement matches an entity if its kind string equals `id`, the
    /// canonical kind string, or any of `applies_to`.
    #[serde(default)]
    pub applies_to: Vec<String>,
    #[serde(default)]
    pub asset: String,
    #[serde(default)]
    pub geometry: GeometrySpec,
    #[serde(default)]
    pub environment: EnvironmentSpec,
    #[serde(default)]
    pub relations: RelationSpec,
    #[serde(default)]
    pub anchors: Vec<AnchorSpec>,
    pub placement: PlacementSpec,
}

impl ObjectDescriptor {
    /// True if this descriptor should govern an entity whose kind string is
    /// `entity_kind`.
    pub fn matches_kind(&self, entity_kind: &str) -> bool {
        entity_kind == self.id
            || self.applies_to.iter().any(|a| a == entity_kind)
            || descriptor_kind_str(self.kind).is_some_and(|k| k == entity_kind)
    }
}

/// ItemKind 鈫?canonical entity kind string (used for matching + reporting).
pub fn descriptor_kind_str(kind: ItemKind) -> Option<&'static str> {
    Some(match kind {
        ItemKind::Road => "road",
        ItemKind::Railway => "railway",
        ItemKind::Building => "building",
        ItemKind::Storefront => "storefront",
        ItemKind::Tree => "tree",
        ItemKind::Rock => "rock",
        ItemKind::Water => "water",
        ItemKind::Bridge => "bridge",
        ItemKind::Park => "park",
        ItemKind::Sidewalk => "sidewalk",
        ItemKind::Lamp => "lamp",
        ItemKind::Bench => "bench",
        ItemKind::BusStop => "bus_stop",
        ItemKind::FoodStall => "food_stall",
        ItemKind::Other => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_minimal_tree() {
        let toml = r#"
id = "tree_common"
kind = "tree"
asset = "assets/vegetation/tree_common.glb"

[geometry]
footprint = [4.0, 4.0]
height = 8.0

[environment]
on_ground = true
not_in_water = true
max_slope = 30

[placement]
phase = "vegetation"
"#;
        let d: ObjectDescriptor = toml::from_str(toml).expect("parse");
        assert_eq!(d.id, "tree_common");
        assert_eq!(d.kind, ItemKind::Tree);
        assert_eq!(d.geometry.footprint, [4.0, 4.0]);
        assert_eq!(d.environment.max_slope, 30.0);
        assert_eq!(d.placement.phase, PlacementPhase::Vegetation);
        assert_eq!(d.placement.priority, 50); // default
        assert_eq!(d.geometry.clearance, 0.5); // default
        assert!(d.relations.avoid.is_empty());
        assert!(d.anchors.is_empty());
    }

    #[test]
    fn deserialize_building_with_anchors() {
        let toml = r#"
id = "residential_house"
kind = "building"
tags = ["residential", "has_entrance"]

[geometry]
footprint = [12.0, 10.0]
height = 7.0

[environment]
on_ground = true
not_in_water = true
max_slope = 12

[[relations.avoid]]
kind = "road"
distance = 0.0

[[relations.avoid]]
kind = "railway"
distance = 20.0

[[relations.prefer]]
kind = "storefront"
distance = 20.0
weight = 0.4

[[anchors]]
id = "front"
side = "front"
kind = "public_access"
clear_radius = 3.0
must_face = "road"

[placement]
phase = "building"
priority = 30
"#;
        let d: ObjectDescriptor = toml::from_str(toml).expect("parse");
        assert_eq!(d.relations.avoid.len(), 2);
        assert_eq!(d.relations.avoid[1].kind, ItemKind::Railway);
        assert_eq!(d.relations.avoid[1].distance, 20.0);
        assert_eq!(d.relations.prefer[0].weight, 0.4);
        assert_eq!(d.anchors.len(), 1);
        assert_eq!(d.anchors[0].id, "front");
        assert_eq!(d.anchors[0].clear_radius, 3.0);
        assert_eq!(d.anchors[0].must_face.as_deref(), Some("road"));
        assert_eq!(d.placement.phase, PlacementPhase::Building);
    }

    #[test]
    fn kind_hardness() {
        assert!(ItemKind::Road.is_hard());
        assert!(ItemKind::Building.is_hard());
        assert!(!ItemKind::Tree.is_hard());
        assert!(!ItemKind::Rock.is_hard());
    }
}



