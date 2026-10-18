//! WHERE an explicit rural-to-urban conversion may happen: the urban edge, and nothing else.
//!
//! Round 30 made the conversion budget honest - it can no longer move more farmland into the urban
//! purpose row than the world planned as urban - but it left the urban purpose free to appear in any
//! field the road happens to cross. The 1 km probe measured what that looks like: 78 converted
//! parcels spread over the whole 1,000 x 1,000 m scene, each one individually legal (its buildable
//! core lies wholly in rural land), none of them a continuation of the city it claims to extend. A
//! real city does not do that. It accretes at its edge: the next row of houses is the row behind the
//! last one, along a street the city already has, and the row after that is behind that. The pattern
//! is called ribbon development and infill; the opposite of it is a settlement pattern with a hole in
//! the middle and a ring of detached shops in the fields.
//!
//! # The band
//!
//! ```text
//! fringe = { cells c : distance(c, nearest urban zone) <= FRINGE_M }
//! ```
//!
//! A conversion is allowed only when the parcel's BUILDABLE CORE (the same rectangle the purpose
//! gate and the audit already measure, `frontage::buildable_core`) lies wholly inside that band.
//! Everything else is refused BY NAME ([`CONVERSION_OUTSIDE_THE_URBAN_FRINGE`]) with the distance
//! the core stood from the nearest urban zone.
//!
//! The distance is the cell metric the project already uses for spatial policy: the Chebyshev
//! distance `max(dx, dz)` between the core rectangle and the zone rectangle, zero when they touch
//! (`worldgen::zoning_distance::Bounds::distance`, which is how the protection policy measures
//! distance to a protected rectangle). Measuring one spatial rule in Euclidean metres and the next in
//! cells would make two published numbers incomparable, so the band is the square dilation of the
//! urban zones - exactly "the cells within FRINGE_M of the city" on this project's 1 m cell grid.
//!
//! # Why 64 m
//!
//! 64 m is this project's own road module, not a round number:
//!
//!  * every road entity the network plans is 64 m long along its axis (`settlement::development::
//!    road_rect`: "a road that is 8 m wide and 64 m long along z owns the same square metres"), and
//!    64 m is the finest rung of the automatic-extension ladder
//!    (`style.automaticRoadExtensions.segmentLengthsM: [512, 256, 128, 64]`). A fringe of one module
//!    therefore means "a conversion may reach exactly one more street segment into the field" -
//!    the accretion unit of the world's own network;
//!  * it is one frontage block face: the frontage programme's finest station spacing is 16 m (four
//!    stations per 64 m), and the authored 512 m samples place 24 x 24 m homes on it, so 64 m is
//!    between two and three house depths - one row of houses behind the existing last row;
//!  * against the 400-900 m pedestrian shed the morphology literature uses for a walkable
//!    neighbourhood, 64 m is 7-16 % of that radius. Every converted parcel therefore stands inside
//!    the walkable catchment of urban land that already exists, and the band can never define a new
//!    city of its own: at 400 m the band would be as deep as the smallest life circle, which would
//!    let a conversion create as much new urban land as the city it claims to extend.
//!
//! The number is a policy constant, not a measurement: it is published on the plan
//! ([`ConversionFringe::fringe_m`]) so a reader can see which band a world was built under, and the
//! audit refuses a plan whose published band disagrees with this constant.
//!
//! # What it does not do
//!
//! The band does not widen the purpose gate: a core that is not wholly rural is still refused
//! (`wrong_purpose_or_protected`), protected land is still refused for the parcel rectangle, and the
//! urban purpose allocation of round 30 still caps the total. It only decides WHERE the same
//! permission applies. With `style.frontageProgram.ruralConversion` off nothing here runs at all, so
//! no world that does not opt in can change.
use crate::worldgen::{
    LandUseZone,
    conversion_budget::plan_conversion_records,
    frontage::{self, Conversion},
    road_network::RoadNetworkPlan,
};
use serde::{Deserialize, Serialize};

/// How far outside an existing urban zone's boundary a conversion may reach, in metres.
///
/// See the module documentation for the argument. This is the ONE definition: the purpose gate, the
/// per-record audit (`frontage::verify_conversion`) and the published record all read it, so they
/// cannot disagree about the width of the band.
pub const FRINGE_M: i64 = 64;

/// The named category one conversion candidate carries when its buildable core stands outside
/// [`FRINGE_M`] of every urban zone.
///
/// It is a stable machine-readable key, not prose: the frontage programme counts it among its
/// request's rejection reasons, and a roadside pad refused under it publishes the same key in
/// `roadsideSiteRejections`.
pub const CONVERSION_OUTSIDE_THE_URBAN_FRINGE: &str = "conversion_outside_the_urban_fringe";

/// One rectangle as the fringe measures it: `[x0, z0, x1, z1]`, half-open, in world metres.
pub type Box4 = [i64; 4];

/// The urban zones' own rectangles, in zoning order.
///
/// The band is built from the zone BOUNDARY, so the zones are used exactly as the zoning publishes
/// them: a conversion may not move a zone's edge, and the fringe is measured against land the world
/// already called urban.
pub fn urban_rects(zones: &[LandUseZone]) -> Vec<Box4> {
    zones
        .iter()
        .filter(|zone| zone.kind == Conversion::URBAN)
        .map(|zone| {
            let x0 = i64::from(zone.world_x);
            let z0 = i64::from(zone.world_z);
            [
                x0,
                z0,
                x0 + i64::from(zone.width_m),
                z0 + i64::from(zone.depth_m),
            ]
        })
        .collect()
}

/// Chebyshev cell distance from one rectangle to one urban zone rectangle, zero when they touch or
/// overlap.
fn rect_distance(b: Box4, zone: Box4) -> i64 {
    let dx = (zone[0] - b[2]).max(b[0] - zone[2]).max(0);
    let dz = (zone[1] - b[3]).max(b[1] - zone[3]).max(0);
    dx.max(dz)
}

/// How far `b` stands from the NEAREST urban zone, in whole metres.
///
/// `None` when the scene plans no urban zone at all: there is then no city for a conversion to
/// extend, and the band does not exist. A caller must treat `None` as "outside the band" - the two
/// are refused under the same name, and a plan that converted land in a world with no urban zone
/// fails the audit by name.
pub fn distance_to_urban(b: Box4, zones: &[LandUseZone]) -> Option<i64> {
    if b[2] <= b[0] || b[3] <= b[1] {
        return None;
    }
    let mut best: Option<i64> = None;
    for zone in urban_rects(zones) {
        let distance = rect_distance(b, zone);
        best = Some(match best {
            Some(current) => current.min(distance),
            None => distance,
        });
        if best == Some(0) {
            break;
        }
    }
    best
}

/// Whether a distance measured by [`distance_to_urban`] lies inside the band.
pub fn within_band(distance: Option<i64>) -> bool {
    distance.is_some_and(|distance| distance <= FRINGE_M)
}

/// The audit's prose for one record or candidate the band refuses, with the distance it stood.
///
/// One function produces the message for the placement gate, the per-record audit and the plan-level
/// audit, so the three can never quote different numbers for the same rectangle.
pub fn outside_message(parcel_id: &str, distance: Option<i64>) -> String {
    match distance {
        Some(distance) => format!(
            "conversion parcel {parcel_id} has its buildable core {distance} m from the nearest \
             urban zone, outside the {FRINGE_M} m urban fringe the explicit conversion policy allows"
        ),
        None => format!(
            "conversion parcel {parcel_id} converts rural land in a scene that plans no urban zone \
             for the conversion to extend"
        ),
    }
}

/// The published summary of what the urban-edge band refused, written only when it refused anything.
///
/// A plan that never had a candidate outside the band publishes no such key at all, which is what
/// keeps a world whose conversions all sit on the urban edge byte-identical to the previous round.
/// The pad programme's own refusals do not appear here: every one of them already publishes a named
/// `roadsideSiteRejections` record with the measured distance in its prose.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversionFringe {
    /// The band this plan converted inside, in metres. Must equal [`FRINGE_M`].
    pub fringe_m: i64,
    /// One entry per frontage request whose candidates the band refused, in program order.
    pub refusals: Vec<FringeRefusal>,
}

/// One request's share of the band's refusals.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FringeRefusal {
    pub request_id: String,
    /// Candidates this request refused for standing outside the band. Equals the request's
    /// `rejected[CONVERSION_OUTSIDE_THE_URBAN_FRINGE]` count, which the audit re-checks.
    pub refused: u32,
    /// How far the NEAREST refused candidate's buildable core stood from the nearest urban zone.
    /// `None` when the scene plans no urban zone at all.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nearest_m: Option<i64>,
}

impl ConversionFringe {
    /// Build the record from the per-request evidence, or `None` when nothing was refused.
    pub fn from_refusals(refusals: Vec<FringeRefusal>) -> Option<Self> {
        if refusals.is_empty() {
            return None;
        }
        Some(Self {
            fringe_m: FRINGE_M,
            refusals,
        })
    }
}

/// Everything an independent audit can re-derive about the urban-edge policy from the plan and the
/// zoning alone. Named failures, never a count that happens to agree.
///
/// Three things are checked, and each is measured here from published geometry instead of read off
/// the plan's own claim:
///
///  * every conversion record the plan keeps has its buildable core inside the band. This is the
///    same test [`frontage::verify_conversion`] applies per record, restated for the case a caller
///    wants the whole list; a record whose core was hand-edited to an out-of-band rectangle fails
///    here by name with the distance it now stands;
///  * every planned roadside pad that DECLARED an urban purpose on wholly-rural land has its core
///    inside the band, so a plan cannot keep a shop in the middle of a field by dropping its
///    conversion record;
///  * a published [`ConversionFringe`] states this build's band, refuses something, quotes a nearest
///    distance that is really outside the band, and agrees with the request outcomes' own counts.
pub fn audit_failures(plan: &RoadNetworkPlan, zones: &[LandUseZone]) -> Vec<String> {
    let mut failures = Vec::new();
    for record in plan_conversion_records(plan) {
        let Some(site) = plan
            .roadside_sites
            .iter()
            .find(|site| site.id == record.parcel_id)
        else {
            // A record whose parcel is not in the plan at all is the ledger audit's own named
            // failure; the band has nothing to measure.
            continue;
        };
        let Some(core) = frontage::site_buildable_core(site, zones) else {
            continue;
        };
        let distance = distance_to_urban(core, zones);
        if !within_band(distance) {
            failures.push(format!(
                "{} (and is still recorded as converting {} m2)",
                outside_message(&record.parcel_id, distance),
                record.area_m2
            ));
        }
    }
    // A pad that declared an urban purpose on wholly-rural land may not stand outside the band: the
    // record list is derived from the final pad list, so a plan that dropped the record while keeping
    // the pad would otherwise book a shop in the fields as ordinary rural cover.
    for site in &plan.roadside_sites {
        if site.purpose.as_deref() != Some(Conversion::URBAN) {
            continue;
        }
        let Some(core) = frontage::site_buildable_core(site, zones) else {
            continue;
        };
        if !frontage::core_is_rural(core, zones) {
            continue;
        }
        let distance = distance_to_urban(core, zones);
        if !within_band(distance) {
            failures.push(format!(
                "pad {} declared urban purpose on wholly-rural land but stands outside the {} m \
                 urban fringe: {}",
                site.id,
                FRINGE_M,
                outside_message(&site.id, distance)
            ));
        }
    }
    let Some(published) = &plan.conversion_fringe else {
        return failures;
    };
    if published.fringe_m != FRINGE_M {
        failures.push(format!(
            "the plan published an urban fringe of {claimed} m but this build allows {FRINGE_M} m",
            claimed = published.fringe_m
        ));
    }
    if published.refusals.is_empty() {
        failures.push(format!(
            "the plan published an empty urban fringe record for a {FRINGE_M} m band"
        ));
    }
    let mut seen = std::collections::BTreeSet::new();
    for refusal in &published.refusals {
        if !seen.insert(refusal.request_id.as_str()) {
            failures.push(format!(
                "frontage request {} publishes two urban-fringe refusals",
                refusal.request_id
            ));
        }
        if refusal.refused == 0 {
            failures.push(format!(
                "frontage request {} publishes an urban-fringe refusal of zero candidates",
                refusal.request_id
            ));
        }
        if let Some(nearest) = refusal.nearest_m {
            if nearest <= FRINGE_M {
                failures.push(format!(
                    "frontage request {} publishes a nearest refused candidate {nearest} m from the \
                     nearest urban zone, which is inside the {FRINGE_M} m band and therefore not a \
                     refusal at all",
                    refusal.request_id
                ));
            }
        }
        let outcome = plan
            .frontage
            .as_ref()
            .and_then(|report| {
                report
                    .outcomes
                    .iter()
                    .find(|outcome| outcome.request.id == refusal.request_id)
            });
        match outcome {
            Some(outcome) => {
                let counted = outcome
                    .rejected
                    .get(CONVERSION_OUTSIDE_THE_URBAN_FRINGE)
                    .copied()
                    .unwrap_or(0);
                if counted != refusal.refused {
                    failures.push(format!(
                        "frontage request {} publishes {} urban-fringe refusals but its outcome \
                         counted {counted}",
                        refusal.request_id, refusal.refused
                    ));
                }
            }
            None => failures.push(format!(
                "frontage request {} publishes an urban-fringe refusal but the report has no outcome \
                 for it",
                refusal.request_id
            )),
        }
    }
    failures
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worldgen::{LandUseZone, frontage::Conversion};

    fn zone(kind: &str, x: i32, z: i32, width: u32, depth: u32) -> LandUseZone {
        LandUseZone {
            id: format!("{kind}-{x}-{z}"),
            kind: kind.into(),
            world_x: x,
            world_z: z,
            width_m: width,
            depth_m: depth,
            source: "test".into(),
        }
    }

    /// The metric is the cell distance the project already uses for spatial policy: the Chebyshev
    /// distance between the two rectangles, zero when they touch or overlap.
    #[test]
    fn the_distance_is_the_cell_metric_and_zero_on_contact() {
        let zones = vec![zone(Conversion::URBAN, 10, 10, 10, 10)];
        // east of the zone, same z: 10 m of empty cells
        assert_eq!(distance_to_urban([30, 12, 40, 20], &zones), Some(10));
        // west of the zone, same z
        assert_eq!(distance_to_urban([0, 15, 5, 25], &zones), Some(5));
        // north-west corner: the metric is max(dx, dz), not the diagonal
        assert_eq!(distance_to_urban([0, 0, 5, 5], &zones), Some(5));
        // a core that TOUCHES the zone edge is at distance 0
        assert_eq!(distance_to_urban([20, 10, 25, 20], &zones), Some(0));
        // an overlapping core is inside the city, distance 0
        assert_eq!(distance_to_urban([15, 15, 25, 25], &zones), Some(0));
        // the nearest of several zones wins
        let two = vec![
            zone(Conversion::URBAN, 0, 0, 10, 10),
            zone(Conversion::URBAN, 100, 100, 10, 10),
        ];
        assert_eq!(distance_to_urban([50, 50, 60, 60], &two), Some(40));
        // no urban zone: there is no edge to measure from, and no band
        assert_eq!(distance_to_urban([50, 50, 60, 60], &[zone("rural", 0, 0, 200, 200)]), None);
        assert_eq!(distance_to_urban([50, 50, 60, 60], &[]), None);
        // a degenerate rectangle is not a place, so it has no distance
        assert_eq!(distance_to_urban([50, 50, 50, 60], &zones), None);
    }

    /// The band is exactly `FRINGE_M` cells wide, and it is 64 m: one road module, one block face.
    #[test]
    fn the_band_is_64_metres_from_the_zone_boundary() {
        assert_eq!(FRINGE_M, 64);
        let zones = vec![zone(Conversion::URBAN, 100, 100, 50, 50)];
        // the core sits `gap` metres east of the zone's right edge (x = 150)
        let at = |gap: i64| [150 + gap, 110, 160 + gap, 120];
        assert!(within_band(distance_to_urban(at(0), &zones)));
        assert!(within_band(distance_to_urban(at(1), &zones)));
        assert!(within_band(distance_to_urban(at(FRINGE_M), &zones)), "64 m is inside");
        let outside = at(FRINGE_M + 1);
        assert!(!within_band(distance_to_urban(outside, &zones)), "65 m is outside");
        assert_eq!(distance_to_urban(outside, &zones), Some(65));
        // "no urban zone at all" is outside the band, not inside it
        assert!(!within_band(None));
    }

    /// The refusal names the parcel, the distance it measured and the band it broke - from ONE
    /// producer, so the gate, the per-record audit and the plan-level audit quote the same numbers.
    #[test]
    fn the_refusal_quotes_the_distance_and_the_band() {
        assert_eq!(
            outside_message("generated.frontage.parcel", Some(77)),
            "conversion parcel generated.frontage.parcel has its buildable core 77 m from the \
             nearest urban zone, outside the 64 m urban fringe the explicit conversion policy allows"
        );
        assert_eq!(
            outside_message("pad.a", None),
            "conversion parcel pad.a converts rural land in a scene that plans no urban zone for \
             the conversion to extend"
        );
    }

    /// The published summary exists only when it refused something, and it states this build's band.
    #[test]
    fn the_published_record_is_written_only_for_a_refusal() {
        assert_eq!(ConversionFringe::from_refusals(vec![]), None);
        let record = ConversionFringe::from_refusals(vec![FringeRefusal {
            request_id: "homes".into(),
            refused: 3,
            nearest_m: Some(97),
        }])
        .expect("a refusal publishes the band");
        assert_eq!(record.fringe_m, FRINGE_M);
        assert_eq!(
            serde_json::to_value(&record).unwrap(),
            serde_json::json!({
                "fringeM": 64,
                "refusals": [{"requestId": "homes", "refused": 3, "nearestM": 97}]
            })
        );
        // A world with no urban zone omits the distance instead of inventing one.
        let record = ConversionFringe::from_refusals(vec![FringeRefusal {
            request_id: "homes".into(),
            refused: 1,
            nearest_m: None,
        }])
        .unwrap();
        assert_eq!(
            serde_json::to_value(&record).unwrap(),
            serde_json::json!({"fringeM": 64, "refusals": [{"requestId": "homes", "refused": 1}]})
        );
    }
}