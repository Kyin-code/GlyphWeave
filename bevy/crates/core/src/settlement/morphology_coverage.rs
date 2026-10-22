//! Read-only consumption of a morphology profile's footprint-coverage range.
//!
//! The profile describes a planning range, not a command to resize already
//! authored parcels. This audit therefore measures the realized parcel and
//! building footprints that the production pipeline actually produced and names
//! every lot outside the profile's range.

use super::development::DevelopmentPlan;
use super::morphology::MorphologyProfile;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CoverageAudit {
    pub format: String,
    pub version: u32,
    pub scene_id: String,
    pub profile_id: String,
    pub target_coverage_ratio: [f64; 2],
    pub parcels: usize,
    pub realized_parcels: usize,
    pub building_footprint_m2: u64,
    pub parcel_area_m2: u64,
    pub weighted_coverage_ratio: f64,
    pub min_parcel_coverage_ratio: Option<f64>,
    pub max_parcel_coverage_ratio: Option<f64>,
    pub out_of_range_parcel_ids: Vec<String>,
    pub unrealized_parcel_ids: Vec<String>,
    pub status: String,
    pub failures: Vec<String>,
}

pub fn audit(plan: &DevelopmentPlan, profile: &MorphologyProfile) -> Result<CoverageAudit, String> {
    let mut building_area = 0_u64;
    let mut parcel_area = 0_u64;
    let mut ratios = Vec::new();
    let mut out_of_range = Vec::new();
    let mut unrealized = Vec::new();

    for parcel in &plan.parcels {
        let area = u64::from(parcel.boundary.width_m)
            .checked_mul(u64::from(parcel.boundary.depth_m))
            .ok_or_else(|| format!("parcel area overflows: {}", parcel.id))?;
        if area == 0 {
            return Err(format!("parcel has zero area: {}", parcel.id));
        }
        let Some(building) = plan
            .buildings
            .iter()
            .find(|building| building.id == parcel.building_id)
        else {
            unrealized.push(parcel.id.clone());
            continue;
        };
        let footprint = u64::from(building.footprint.width_m)
            .checked_mul(u64::from(building.footprint.depth_m))
            .ok_or_else(|| format!("building footprint overflows: {}", building.id))?;
        if footprint == 0 {
            return Err(format!("building footprint has zero area: {}", building.id));
        }
        let ratio = footprint as f64 / area as f64;
        if ratio < profile.coverage_ratio[0] - 1e-9 || ratio > profile.coverage_ratio[1] + 1e-9 {
            out_of_range.push(parcel.id.clone());
        }
        building_area = building_area
            .checked_add(footprint)
            .ok_or_else(|| "building footprint total overflows".to_string())?;
        parcel_area = parcel_area
            .checked_add(area)
            .ok_or_else(|| "parcel area total overflows".to_string())?;
        ratios.push(ratio);
    }

    let min_ratio = ratios.iter().copied().reduce(f64::min);
    let max_ratio = ratios.iter().copied().reduce(f64::max);
    let weighted = if parcel_area == 0 {
        0.
    } else {
        building_area as f64 / parcel_area as f64
    };
    let mut failures = Vec::new();
    if !unrealized.is_empty() {
        failures.push("planned parcels have no realized building".into());
    }
    if !out_of_range.is_empty() {
        failures.push("realized parcel coverage is outside the profile range".into());
    }
    Ok(CoverageAudit {
        format: "glyphweave.morphology-coverage-audit".into(),
        version: 1,
        scene_id: plan.scene_id.clone(),
        profile_id: profile.profile_id.clone(),
        target_coverage_ratio: profile.coverage_ratio,
        parcels: plan.parcels.len(),
        realized_parcels: ratios.len(),
        building_footprint_m2: building_area,
        parcel_area_m2: parcel_area,
        weighted_coverage_ratio: weighted,
        min_parcel_coverage_ratio: min_ratio,
        max_parcel_coverage_ratio: max_ratio,
        out_of_range_parcel_ids: out_of_range,
        unrealized_parcel_ids: unrealized,
        status: if failures.is_empty() && !ratios.is_empty() {
            "pass".into()
        } else {
            "warn".into()
        },
        failures,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settlement::development::{BuildingPlan, DevelopmentPlan, FrontageParcel, Rect};
    use crate::worldgen::EntityInstance;
    use std::collections::BTreeMap;

    fn entity(id: &str) -> EntityInstance {
        serde_json::from_value(serde_json::json!({
            "entityId": id,
            "assetId": "building.residential",
            "kind": "residential_home",
            "worldX": 10,
            "worldZ": 10,
            "worldY": 0,
            "widthM": 10,
            "depthM": 10,
            "heightM": 8,
            "rotationYDeg": 0,
            "scale": 1
        }))
        .unwrap()
    }

    fn plan(footprint_width: u32, footprint_depth: u32) -> DevelopmentPlan {
        DevelopmentPlan {
            version: 1,
            source: "test".into(),
            scene_id: "scene-0".into(),
            natural_heightfield_hash: String::new(),
            scene_area_m2: 10_000,
            road_ids: vec!["road".into()],
            parcels: vec![FrontageParcel {
                id: "parcel".into(),
                road_id: "road".into(),
                building_id: "building".into(),
                boundary: Rect {
                    x: 0,
                    z: 0,
                    width_m: 100,
                    depth_m: 100,
                },
                footprint_m2: u64::from(footprint_width) * u64::from(footprint_depth),
                yard_m2: 0,
                parcel_insets: Default::default(),
                source_zones: BTreeMap::new(),
            }],
            buildings: vec![BuildingPlan {
                id: "building".into(),
                parcel_id: "parcel".into(),
                road_id: "road".into(),
                footprint: Rect {
                    x: 10,
                    z: 10,
                    width_m: footprint_width,
                    depth_m: footprint_depth,
                },
                entity: entity("building"),
            }],
            cells: vec![],
        }
    }

    fn profile(range: [f64; 2]) -> MorphologyProfile {
        let mut profile = crate::settlement::morphology::builtins()
            .into_iter()
            .find(|profile| profile.profile_id == "village-field")
            .unwrap();
        profile.coverage_ratio = range;
        profile
    }

    #[test]
    fn realized_parcel_ratio_inside_the_profile_passes() {
        let audit = audit(&plan(50, 100), &profile([0.45, 0.55])).unwrap();
        assert_eq!(audit.status, "pass");
        assert_eq!(audit.realized_parcels, 1);
        assert_eq!(audit.weighted_coverage_ratio, 0.5);
        assert!(audit.out_of_range_parcel_ids.is_empty());
    }

    #[test]
    fn an_out_of_range_or_unrealized_parcel_is_named() {
        let out = audit(&plan(20, 100), &profile([0.45, 0.55])).unwrap();
        assert_eq!(out.status, "warn");
        assert_eq!(out.out_of_range_parcel_ids, vec!["parcel"]);
        let mut missing = plan(50, 100);
        missing.buildings.clear();
        let missing = audit(&missing, &profile([0.45, 0.55])).unwrap();
        assert_eq!(missing.status, "warn");
        assert_eq!(missing.unrealized_parcel_ids, vec!["parcel"]);
    }
}
