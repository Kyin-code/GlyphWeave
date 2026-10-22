//! Deterministic building archetypes used by demand-led settlement planning.
//!
//! The archetype controls morphology (footprint range, floor range, coverage and
//! recommended density count); household/job/retail demand still decides how
//! much capacity is occupied and whether another parcel is needed.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SelectionInput {
    pub community_profile: String,
    pub district_stage: String,
    pub center_pressure: f64,
    pub lot_width_m: f32,
    pub lot_depth_m: f32,
    pub needs_homes: bool,
    pub needs_jobs: bool,
    pub needs_retail: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Archetype {
    pub id: String,
    pub kind: String,
    pub min_width_m: f32,
    pub max_width_m: f32,
    pub min_depth_m: f32,
    pub max_depth_m: f32,
    pub min_floors: u32,
    pub max_floors: u32,
    pub coverage_ratio: f32,
    pub recommended_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BuildingForm {
    pub archetype_id: String,
    pub kind: String,
    pub width_m: f32,
    pub depth_m: f32,
    pub floors: u32,
    pub coverage_ratio: f32,
    pub footprint_m2: f32,
    pub recommended_count: u32,
}

pub fn choose(input: &SelectionInput) -> Archetype {
    // Near the strongest network centre, employment can dominate the lot.
    // Away from it, mixed residential demand keeps the block residential and
    // lets the floor-area programme supply its ground-floor retail share.
    if input.needs_jobs && input.center_pressure > 0.78 {
        return archetype(
            "office-tower-podium",
            "office_tower",
            18.,
            42.,
            18.,
            42.,
            10,
            42,
            0.56,
            1,
        );
    }
    if input.needs_homes && input.needs_jobs {
        return archetype(
            "mixed-use-midrise",
            "residential_block",
            10.,
            26.,
            10.,
            22.,
            4,
            10,
            0.70,
            2,
        );
    }
    if input.needs_jobs && input.center_pressure > 0.55 {
        return archetype(
            "office-tower-podium",
            "office_tower",
            14.,
            32.,
            14.,
            32.,
            8,
            32,
            0.62,
            1,
        );
    }
    if input.needs_retail
        && input.center_pressure > 0.52
        && (!input.needs_homes || input.center_pressure > 0.70)
    {
        return retail_archetype(input);
    }
    if input.needs_homes || !input.needs_jobs {
        return residential_archetype(input);
    }
    if input.needs_retail {
        return retail_archetype(input);
    }
    archetype(
        "office-tower-podium",
        "office_tower",
        18.,
        42.,
        18.,
        42.,
        10,
        42,
        0.56,
        1,
    )
}

fn residential_archetype(input: &SelectionInput) -> Archetype {
    if input.district_stage == "old-core" {
        return match input.community_profile.as_str() {
            "japan-machiya" => archetype(
                "machiya-row",
                "residential_home",
                4.,
                12.,
                8.,
                22.,
                2,
                4,
                0.76,
                3,
            ),
            "europe-perimeter" => archetype(
                "historic-perimeter-wing",
                "residential_home",
                18.,
                46.,
                14.,
                28.,
                3,
                7,
                0.76,
                2,
            ),
            _ => archetype(
                "old-core-lane-house",
                "residential_home",
                5.,
                12.,
                8.,
                22.,
                2,
                5,
                0.82,
                3,
            ),
        };
    }
    if input.district_stage == "new-growth" {
        return archetype(
            "new-town-midrise",
            "residential_home",
            18.,
            34.,
            14.,
            26.,
            6,
            18,
            0.52,
            2,
        );
    }
    if input.district_stage == "renewal" {
        return archetype(
            "renewal-block",
            "residential_home",
            12.,
            28.,
            12.,
            24.,
            4,
            12,
            0.68,
            2,
        );
    }
    match input.community_profile.as_str() {
        "us-suburban" => archetype(
            "detached-house",
            "residential_home",
            8.,
            18.,
            10.,
            26.,
            1,
            3,
            0.28,
            1,
        ),
        "hong-kong-estate" => archetype(
            "estate-podium",
            "residential_home",
            16.,
            32.,
            14.,
            26.,
            10,
            28,
            0.58,
            2,
        ),
        "japan-machiya" if input.center_pressure > 0.62 => archetype(
            "machiya-row",
            "residential_home",
            4.,
            12.,
            8.,
            22.,
            2,
            4,
            0.76,
            3,
        ),
        "china-gated" if input.center_pressure < 0.62 => archetype(
            "gated-residential-slab",
            "residential_home",
            28.,
            64.,
            12.,
            26.,
            6,
            20,
            0.28,
            2,
        ),
        "china-urban-village" => archetype(
            "urban-village-row",
            "residential_home",
            8.,
            20.,
            10.,
            28.,
            3,
            8,
            0.68,
            2,
        ),
        "europe-perimeter" | "us-mixed-income" => archetype(
            "perimeter-block",
            "residential_home",
            24.,
            68.,
            16.,
            34.,
            4,
            9,
            0.66,
            1,
        ),
        _ => archetype(
            "midrise-apartment",
            "residential_home",
            18.,
            48.,
            14.,
            32.,
            5,
            14,
            0.52,
            1,
        ),
    }
}

fn retail_archetype(input: &SelectionInput) -> Archetype {
    match input.community_profile.as_str() {
        "japan-machiya" => archetype(
            "machiya-street",
            "storefront",
            4.,
            12.,
            8.,
            22.,
            2,
            4,
            0.78,
            1,
        ),
        "europe-perimeter" => archetype(
            "perimeter-shopfront",
            "storefront",
            24.,
            70.,
            14.,
            30.,
            4,
            8,
            0.68,
            1,
        ),
        _ if input.center_pressure > 0.58 => archetype(
            "midrise-shopfront",
            "storefront",
            10.,
            24.,
            10.,
            22.,
            2,
            6,
            0.72,
            2,
        ),
        _ => archetype(
            "neighbourhood-shopfront",
            "storefront",
            10.,
            24.,
            10.,
            26.,
            2,
            4,
            0.62,
            1,
        ),
    }
}

pub fn form(key: &str, input: &SelectionInput, profile: &Archetype) -> BuildingForm {
    let width = vary(key, "width", profile.min_width_m, profile.max_width_m)
        .min((input.lot_width_m * 0.9).max(4.0))
        .max(4.0);
    let depth = vary(key, "depth", profile.min_depth_m, profile.max_depth_m)
        .min((input.lot_depth_m * 0.9).max(4.0))
        .max(4.0);
    let floor_span = profile.max_floors.saturating_sub(profile.min_floors);
    let floor_t = unit(key, "floors");
    let pressure_floor = (profile.min_floors as f64
        + floor_span as f64 * (0.35 + input.center_pressure * 0.65) * floor_t)
        .round() as u32;
    let floors = pressure_floor.clamp(profile.min_floors, profile.max_floors);
    let footprint_m2 = width * depth * profile.coverage_ratio;
    let kind = match (profile.kind.as_str(), floors) {
        ("residential_home", 18..) => "residential_tower",
        ("residential_home", 7..=17) => "residential_block",
        ("storefront", 9..) => "urban_building",
        (kind, _) => kind,
    };
    BuildingForm {
        archetype_id: profile.id.clone(),
        kind: kind.into(),
        width_m: width.max(4.0),
        depth_m: depth.max(4.0),
        floors,
        coverage_ratio: profile.coverage_ratio,
        footprint_m2,
        recommended_count: profile.recommended_count,
    }
}

#[allow(clippy::too_many_arguments)]
fn archetype(
    id: &str,
    kind: &str,
    min_width_m: f32,
    max_width_m: f32,
    min_depth_m: f32,
    max_depth_m: f32,
    min_floors: u32,
    max_floors: u32,
    coverage_ratio: f32,
    recommended_count: u32,
) -> Archetype {
    Archetype {
        id: id.into(),
        kind: kind.into(),
        min_width_m,
        max_width_m,
        min_depth_m,
        max_depth_m,
        min_floors,
        max_floors,
        coverage_ratio,
        recommended_count,
    }
}

fn vary(key: &str, field: &str, minimum: f32, maximum: f32) -> f32 {
    (minimum + (maximum - minimum) * unit(key, field) as f32).clamp(minimum, maximum)
}

fn unit(key: &str, field: &str) -> f64 {
    let hash = blake3::hash(format!("{key}:{field}").as_bytes());
    u32::from_le_bytes(hash.as_bytes()[..4].try_into().expect("four bytes")) as f64
        / u32::MAX as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(profile: &str, pressure: f64) -> SelectionInput {
        SelectionInput {
            community_profile: profile.into(),
            district_stage: "periphery".into(),
            center_pressure: pressure,
            lot_width_m: 24.0,
            lot_depth_m: 26.0,
            needs_homes: true,
            needs_jobs: false,
            needs_retail: false,
        }
    }

    #[test]
    fn regional_archetypes_change_morphology() {
        let machiya = choose(&input("japan-machiya", 0.8));
        let suburb = choose(&input("us-suburban", 0.2));
        let estate = choose(&input("hong-kong-estate", 0.8));
        assert_eq!(machiya.kind, "residential_home");
        assert_eq!(machiya.max_width_m, 12.0);
        assert_eq!(suburb.id, "detached-house");
        assert_eq!(estate.id, "estate-podium");
    }

    #[test]
    fn form_is_deterministic_and_respects_lot_and_pressure() {
        let mut input = input("europe-perimeter", 0.9);
        input.lot_width_m = 60.0;
        input.lot_depth_m = 60.0;
        let archetype = choose(&input);
        let first = form("lot.4", &input, &archetype);
        let second = form("lot.4", &input, &archetype);
        let other = form("lot.5", &input, &archetype);
        assert_eq!(first, second);
        assert!(first.width_m <= 60.0 * 0.9);
        assert!(first.depth_m <= 60.0 * 0.9);
        assert!(first.floors >= archetype.min_floors);
        assert!(first.floors <= archetype.max_floors);
        assert!(first.footprint_m2 > 0.0);
        assert_ne!(first.width_m, other.width_m);
    }

    #[test]
    fn employment_and_retail_select_non_residential_archetypes() {
        let mut jobs = input("china-gated", 0.9);
        jobs.needs_jobs = true;
        jobs.needs_homes = false;
        assert_eq!(choose(&jobs).kind, "office_tower");

        let mut retail = input("japan-machiya", 0.75);
        retail.needs_retail = true;
        assert_eq!(choose(&retail).kind, "storefront");
    }

    #[test]
    fn mixed_demand_does_not_turn_every_parcel_into_retail() {
        let mut mixed = input("china-gated", 0.58);
        mixed.needs_retail = true;
        let block = choose(&mixed);
        assert_eq!(block.kind, "residential_home");

        mixed.center_pressure = 0.76;
        assert_eq!(choose(&mixed).kind, "storefront");

        mixed.center_pressure = 0.92;
        mixed.needs_jobs = true;
        assert_eq!(choose(&mixed).kind, "office_tower");
    }

    #[test]
    fn district_stage_changes_parcel_morphology() {
        let mut old = input("china-gated", 0.5);
        old.district_stage = "old-core".into();
        assert_eq!(choose(&old).id, "old-core-lane-house");

        let mut renewal = old.clone();
        renewal.district_stage = "renewal".into();
        assert_eq!(choose(&renewal).id, "renewal-block");

        let mut new = renewal.clone();
        new.district_stage = "new-growth".into();
        assert_eq!(choose(&new).id, "new-town-midrise");
    }
}
