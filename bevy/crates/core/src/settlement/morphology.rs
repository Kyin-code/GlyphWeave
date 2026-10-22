//! Rust-owned morphology profiles. Profiles are parameter presets for the
//! existing parcel/road/service pipeline, never renderer-side country rules.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MorphologyProfile {
    pub profile_id: String,
    pub version: u32,
    pub source_case_ids: Vec<String>,
    pub evidence_level: String,
    pub terrain: Vec<String>,
    pub parcel_width_m: [u32; 2],
    pub parcel_depth_m: [u32; 2],
    pub front_inset_m: [u32; 2],
    pub side_inset_m: [u32; 2],
    pub rear_inset_m: [u32; 2],
    pub height_m: [u32; 2],
    pub coverage_ratio: [f64; 2],
    pub road_hierarchy: Vec<String>,
    pub public_space_ratio: [f64; 2],
    pub service_radius_m: u32,
    pub village_field_template: Option<[u32; 2]>,
}
impl MorphologyProfile {
    fn valid(&self) -> Result<(), String> {
        if self.profile_id.is_empty()
            || self.version != 1
            || self.source_case_ids.is_empty()
            || !matches!(
                self.evidence_level.as_str(),
                "observed" | "heuristic" | "source_pending"
            )
            || self.terrain.is_empty()
            || self.road_hierarchy.len() < 2
            || self
                .road_hierarchy
                .iter()
                .any(|r| !matches!(r.as_str(), "collector" | "local" | "farm_access"))
        {
            return Err("invalid morphology profile identity/evidence/road hierarchy".into());
        }
        for range in [self.parcel_width_m, self.parcel_depth_m, self.height_m] {
            if range[0] == 0 || range[0] > range[1] || range[1] > 256 {
                return Err("invalid morphology integer range".into());
            }
        }
        for range in [self.front_inset_m, self.side_inset_m, self.rear_inset_m] {
            if range[0] > range[1] || range[1] > 256 {
                return Err("invalid morphology inset range".into());
            }
        }
        for range in [self.coverage_ratio, self.public_space_ratio] {
            if !range
                .iter()
                .all(|v| v.is_finite() && (0. ..=1.).contains(v))
                || range[0] > range[1]
            {
                return Err("invalid morphology ratio range".into());
            }
        }
        if self.service_radius_m < 100 || self.service_radius_m > 2000 {
            return Err("invalid morphology service radius".into());
        }
        Ok(())
    }
}
fn profile(
    id: &str,
    cases: &[&str],
    terrain: &[&str],
    width: [u32; 2],
    depth: [u32; 2],
    front: [u32; 2],
    side: [u32; 2],
    rear: [u32; 2],
    height: [u32; 2],
    coverage: [f64; 2],
    roads: &[&str],
    public_space: [f64; 2],
    service_radius_m: u32,
    village_field_template: Option<[u32; 2]>,
) -> MorphologyProfile {
    MorphologyProfile {
        profile_id: id.into(),
        version: 1,
        source_case_ids: cases.iter().map(|s| (*s).into()).collect(),
        evidence_level: "heuristic".into(),
        terrain: terrain.iter().map(|s| (*s).into()).collect(),
        parcel_width_m: width,
        parcel_depth_m: depth,
        front_inset_m: front,
        side_inset_m: side,
        rear_inset_m: rear,
        height_m: height,
        coverage_ratio: coverage,
        road_hierarchy: roads.iter().map(|s| (*s).into()).collect(),
        public_space_ratio: public_space,
        service_radius_m,
        village_field_template,
    }
}
pub fn builtins() -> Vec<MorphologyProfile> {
    vec![
        profile(
            "compact-mixed-block",
            &["cn-beijing", "us-new-york", "eu-paris"],
            &["plain", "river-delta"],
            [12, 24],
            [16, 32],
            [2, 6],
            [0, 4],
            [2, 8],
            [8, 24],
            [0.45, 0.75],
            &["collector", "local"],
            [0.05, 0.15],
            600,
            None,
        ),
        profile(
            "perimeter-block",
            &["eu-berlin", "eu-barcelona", "cn-shanghai"],
            &["plain", "coast"],
            [24, 60],
            [32, 80],
            [2, 8],
            [2, 8],
            [12, 32],
            [12, 36],
            [0.35, 0.65],
            &["collector", "local"],
            [0.10, 0.25],
            700,
            None,
        ),
        profile(
            "low-density-residential",
            &["us-los-angeles", "us-denver", "eu-zurich"],
            &["plain", "mountain-foot"],
            [16, 36],
            [28, 72],
            [4, 12],
            [4, 12],
            [12, 40],
            [6, 16],
            [0.15, 0.40],
            &["collector", "local"],
            [0.08, 0.20],
            900,
            None,
        ),
        profile(
            "clustered-residential",
            &["cn-shenzhen", "hk-sha-tin", "jp-tokyo-bay"],
            &["plain", "coast", "mountain-foot"],
            [20, 48],
            [24, 64],
            [4, 10],
            [2, 8],
            [8, 24],
            [10, 40],
            [0.25, 0.55],
            &["collector", "local"],
            [0.12, 0.30],
            700,
            None,
        ),
        profile(
            "terrain-constrained-compact",
            &["cn-chongqing", "cn-qingdao", "hk-central"],
            &["mountain-valley", "coast"],
            [12, 28],
            [16, 40],
            [2, 6],
            [0, 4],
            [4, 12],
            [12, 48],
            [0.40, 0.70],
            &["collector", "local"],
            [0.05, 0.15],
            600,
            None,
        ),
        profile(
            "village-field",
            &["cn-chengdu", "eu-milan", "jp-kyoto-gion"],
            &["plain", "river-delta"],
            [14, 32],
            [20, 48],
            [4, 10],
            [2, 8],
            [10, 32],
            [6, 18],
            [0.20, 0.50],
            &["collector", "local", "farm_access"],
            [0.08, 0.20],
            800,
            Some([24, 24]),
        ),
    ]
}
pub fn resolve(style: &serde_json::Value) -> Result<Option<MorphologyProfile>, String> {
    let id = match style.get("morphologyProfile") {
        None => return Ok(None),
        Some(serde_json::Value::String(id)) => id,
        Some(_) => {
            return Err("morphologyProfile must be a string when present".into());
        }
    };
    let profile = builtins()
        .into_iter()
        .find(|profile| profile.profile_id == *id)
        .ok_or_else(|| format!("unknown morphologyProfile {id}"))?;
    profile.valid()?;
    Ok(Some(profile))
}
pub fn validate_manifest(style: &serde_json::Value) -> Result<(), String> {
    resolve(style)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn six_profiles_are_distinct_valid_and_unknown_ids_fail() {
        let profiles = builtins();
        assert_eq!(profiles.len(), 6);
        for profile in &profiles {
            profile.valid().unwrap();
        }
        assert_ne!(profiles[0].parcel_depth_m, profiles[5].parcel_depth_m);
        assert!(
            profiles
                .iter()
                .any(|profile| profile.village_field_template == Some([24, 24]))
        );
        let known: std::collections::BTreeSet<&str> = [
            "cn-beijing",
            "cn-shanghai",
            "cn-guangzhou",
            "cn-shenzhen",
            "cn-chengdu",
            "cn-chongqing",
            "cn-xian",
            "cn-hangzhou",
            "cn-wuhan",
            "cn-qingdao",
            "cn-xiamen",
            "cn-kunming",
            "us-new-york",
            "us-san-francisco",
            "us-los-angeles",
            "us-chicago",
            "us-seattle",
            "us-boston",
            "us-miami",
            "us-denver",
            "us-portland",
            "us-washington-dc",
            "us-las-vegas",
            "us-new-orleans",
            "eu-london",
            "eu-paris",
            "eu-amsterdam",
            "eu-berlin",
            "eu-zurich",
            "eu-milan",
            "eu-rome",
            "eu-barcelona",
            "eu-lisbon",
            "eu-stockholm",
            "eu-prague",
            "eu-vienna",
            "jp-tokyo-shitamachi",
            "jp-tokyo-bay",
            "jp-osaka-namba",
            "jp-kyoto-gion",
            "jp-yokohama-minato",
            "jp-kobe-sannomiya",
            "hk-central",
            "hk-mong-kok",
            "hk-causeway-bay",
            "hk-kwun-tong",
            "hk-sha-tin",
            "hk-sai-kung",
        ]
        .into_iter()
        .collect();
        assert!(profiles.iter().all(|profile| {
            profile
                .source_case_ids
                .iter()
                .all(|case_id| known.contains(case_id.as_str()))
        }));
        assert!(resolve(&serde_json::json!({"morphologyProfile":"unknown"})).is_err());
        for invalid in [
            serde_json::json!(42),
            serde_json::json!({"profile":"village-field"}),
            serde_json::Value::Null,
            serde_json::json!(["village-field"]),
        ] {
            assert!(resolve(&serde_json::json!({"morphologyProfile": invalid})).is_err());
        }
        assert!(resolve(&serde_json::json!({})).unwrap().is_none());
    }
}
