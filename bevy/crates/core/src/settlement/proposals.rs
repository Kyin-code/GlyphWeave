//! Candidate requests which cannot bypass authoritative development plans.
//! This is diagnostic generation history, NOT evidence of realized services.
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DevelopmentProposal {
    pub proposal_id: String,
    pub kind: String,
    pub world_x: i32,
    pub world_z: i32,
    pub width_m: f32,
    pub depth_m: f32,
    pub height_m: f32,
    pub decision: String,
    pub reason: String,
}
pub fn deferred(
    scene_id: &str,
    serial: usize,
    kind: &str,
    x: i32,
    z: i32,
    w: f32,
    d: f32,
    h: f32,
) -> DevelopmentProposal {
    DevelopmentProposal {
        proposal_id: format!("{scene_id}.legacy-development-request.{serial}"),
        kind: kind.into(),
        world_x: x,
        world_z: z,
        width_m: w,
        depth_m: d,
        height_m: h,
        decision: "deferred-before-entity-generation".into(),
        reason: if matches!(kind, "canal" | "railway" | "parking_lot") {
            "requires-infrastructure-and-land-ownership-plan"
        } else {
            "requires-building-plan-and-legal-frontage-parcel"
        }
        .into(),
    }
}
