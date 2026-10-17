//! Explicit service relations are compiled from morphology profiles and
//! audited against the existing final AccessGraph; Euclidean distance is not
//! acceptance evidence.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceRelation {
    pub service_kind: String,
    pub anchor_kind: String,
    pub max_path_distance_m: u32,
    pub requires_public_space: bool,
}
pub fn defaults(profile_id: &str) -> Vec<ServiceRelation> {
    let radius = super::morphology::builtins()
        .into_iter()
        .find(|profile| profile.profile_id == profile_id)
        .map_or(800, |profile| profile.service_radius_m);
    vec![
        ServiceRelation {
            service_kind: "school".into(),
            anchor_kind: "residential".into(),
            max_path_distance_m: radius,
            requires_public_space: false,
        },
        ServiceRelation {
            service_kind: "market".into(),
            anchor_kind: "public_space".into(),
            max_path_distance_m: radius,
            requires_public_space: true,
        },
        ServiceRelation {
            service_kind: "town_hall".into(),
            anchor_kind: "public_space".into(),
            max_path_distance_m: radius,
            requires_public_space: true,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn relations_use_profile_radius_and_require_real_open_space() {
        let a = defaults("compact-mixed-block");
        let b = defaults("low-density-residential");
        assert_eq!(a[0].max_path_distance_m, 600);
        assert_eq!(b[0].max_path_distance_m, 900);
        assert!(a.iter().filter(|r| r.requires_public_space).count() >= 2);
    }
}
