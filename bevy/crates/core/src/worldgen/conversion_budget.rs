//! The urban purpose allocation one world's explicit rural-to-urban conversions may spend.
//!
//! A conversion moves purpose realization from the rural row to the urban row of the land-use
//! ledger. That is only honest while the urban row can still absorb it: the ledger refuses any
//! purpose whose realization exceeds what was planned (`ledger purpose overcredited`), and the round
//! 29 probe measured exactly that failure - a 1 km world converted 48,548 m2 of farmland into "urban"
//! on top of 9,755 m2 of urban physical cover, against a plan that allocated 40,898 m2 of urban land.
//! The conversions were individually legal (each parcel's buildable core is wholly rural) and jointly
//! impossible. A conversion therefore has to be capped WHERE IT IS DECIDED, not shaved off the ledger
//! afterwards.
//!
//! # The allocation, exactly
//!
//! ```text
//! planned_urban_m2    = sum over land_use_zones of kind "urban" of width_m * depth_m
//! committed_urban_m2  = sum over the plan's committed hard-development rectangles of
//!                       area(rectangle intersected with the urban zones)
//! residual_landcover_m2 = ceil(urban_open_space_ratio * planned_urban_m2)
//! allowance_m2        = planned_urban_m2 - committed_urban_m2 - residual_landcover_m2  (floored 0)
//! headroom_m2         = allowance_m2 - converted_so_far_m2 - reserved_for_pads_m2
//! ```
//!
//! `planned_urban_m2` is the very number the ledger books as `byPurpose.urban.plannedM2`: the same
//! zones, summed in whole square metres, so the two cannot disagree.
//!
//! `committed_urban_m2` counts the two kinds of rectangle the development partition
//! (`settlement::development::plan`) owns urban land with, measured by that module's own
//! `road_rect` / `parcel_rect` helpers: every road entity's roadbed rectangle and every roadside
//! site's frontage-parcel rectangle (building plus its yard). That is deliberately an OVER-estimate
//! of the hard physical cover the ledger will book, which is why the cap is safe: the partition
//! splits each urban zone into Road, Building, Yard and Unplanned cells drawn from exactly those
//! rectangles, and the landcover realization can only cover a SUBSET of the Road/Building/Yard cells
//! (a yard whose building or access route is not realized produces no surface at all).
//!
//! `residual_landcover_m2` is the one component of urban physical cover that does NOT stand on a
//! parcel: the residual-land policy realizes public open space on the urban zone's own remainder
//! (`LandcoverRole::FutureDevelopment` cells, `settlement::landcover::realize`). That share is at
//! most `ratio * unplanned urban land` and therefore at most `ratio * planned_urban_m2`, which is
//! what is reserved here. A world whose effective style states no residual programme reserves
//! nothing.
//!
//! Together, the ledger's urban physical cover is always at most
//! `committed_urban_m2 + residual_landcover_m2`, and therefore
//!
//! ```text
//! urban physical + all conversions <= (committed + residual) + allowance = planned
//! ```
//!
//! holds by construction, whatever the later stages decide. Overlapping rectangles are counted once
//! each even when they share land, which can only make the estimate larger, never smaller.
//!
//! # When it is computed
//!
//! Once per world, from the plan as it stands when the first conversion is about to be decided: all
//! roads are final (extensions and automatic roads are planned before the frontage programme), all
//! roadside pads are final, and the frontage parcels are exactly the ones about to be placed. The
//! account is then spent in PRODUCTION ORDER - the authored pad programme's conversions are reserved
//! first (they are the world's own authored facilities and the frontage programme's demand-led lots
//! must not displace them), then the frontage programme places its lots against the rest, then the
//! pads are granted what was reserved. Every consumer shares the one running account, so no stage can
//! spend area another stage already spent.
//!
//! A candidate that does not fit is refused BY NAME
//! ([`CONVERSION_EXCEEDS_URBAN_ALLOCATION`]) with the area it needs and the area that was left. The
//! refusal record travels with the plan, and no conversion record, ledger row or published surface
//! ever exists for it.
//!
//! The published account distinguishes what it GRANTED from what the plan KEPT. A parcel refused
//! after it was granted - a pad whose declared flat top the plan's own earthwork cannot hold - takes
//! its own conversion record out of the plan through `road_network::forget_refused_pad`, and the
//! account never credits that area back to a later candidate. The audit therefore requires
//! `kept <= granted <= allowance`, and the ledger books exactly the kept records.
use crate::settlement::development::{self, Rect};
use crate::worldgen::{
    LandUseZone, SceneSpec,
    frontage::Conversion,
    road_network::{RoadNetworkPlan, RoadsideSite},
};
use serde::{Deserialize, Serialize};

/// The named category one refused conversion candidate carries.
///
/// It is a stable machine-readable key, not prose: the frontage programme counts it among its
/// request's rejection reasons, and a pad refused under it is withdrawn from the plan exactly like a
/// pad refused for an unrealizable flat top (see `road_network::PAD_REFUSED_CATEGORIES`).
pub const CONVERSION_EXCEEDS_URBAN_ALLOCATION: &str =
    "purpose_conversion_exceeds_urban_allocation";

/// One conversion candidate the allocation refused, with the evidence a reader needs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversionRefusal {
    /// The parcel (frontage lot or roadside pad) that asked for the conversion.
    pub parcel_id: String,
    /// The land it would have moved from rural to urban purpose, in whole square metres.
    pub required_m2: i64,
    /// What the allocation still had when it asked. Always smaller than `required_m2`.
    pub remaining_m2: i64,
}

/// The published record of one plan's urban allocation, written only when it refused something.
///
/// A plan that never hit the cap publishes no such key at all, which is what keeps every already
/// generated world byte-identical.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversionAllocation {
    pub planned_urban_m2: i64,
    /// Planned urban land already committed to roads and parcels when the account was opened.
    pub committed_urban_m2: i64,
    /// The public open space the residual-land policy may still realize inside the urban zones.
    pub residual_landcover_m2: i64,
    /// Conversion area the allocation GRANTED. A parcel that engineering refuses after it was
    /// granted (a pad whose declared flat top the plan's own earthwork cannot hold) takes its own
    /// conversion record out of the plan, so the plan may keep LESS than this - never more. The
    /// account never credits returned area back to a later candidate, which is why this number stays
    /// the conservative "what was spent" total rather than a running total of kept records.
    pub converted_m2: i64,
    /// Conversion area the authored pad programme reserved before the frontage programme ran.
    #[serde(default)]
    pub reserved_for_pads_m2: i64,
    pub refusals: Vec<ConversionRefusal>,
}

impl ConversionAllocation {
    /// The conversion area the allocation allows in total.
    pub fn allowance_m2(&self) -> i64 {
        (self.planned_urban_m2 - self.committed_urban_m2 - self.residual_landcover_m2).max(0)
    }
}

/// Planned urban area: the exact figure the land-use ledger books as the urban row's plan.
pub fn planned_urban_m2(zones: &[LandUseZone]) -> i64 {
    urban_zone_rects(zones).iter().map(|zone| zone.area() as i64).sum()
}

fn urban_zone_rects(zones: &[LandUseZone]) -> Vec<Rect> {
    zones
        .iter()
        .filter(|zone| zone.kind == Conversion::URBAN)
        .map(|zone| Rect {
            x: i64::from(zone.world_x),
            z: i64::from(zone.world_z),
            width_m: zone.width_m,
            depth_m: zone.depth_m,
        })
        .collect()
}

fn clipped_m2(rect: Rect, urban: &[Rect]) -> i64 {
    urban
        .iter()
        .map(|zone| rect.intersection_area(*zone) as i64)
        .sum()
}

/// The public-space share the scene's effective residual-land policy may take inside the urban
/// zones, read from the same compiled style the landcover plan is built from.
pub fn residual_open_space_ratio(style: &serde_json::Value, scene: &SceneSpec) -> Result<f64, String> {
    Ok(crate::settlement::residual_land::for_scene(style, scene)?
        .map(|program| program.urban_open_space_ratio())
        .unwrap_or(0.0)
        .clamp(0.0, 1.0))
}

/// Urban land the residual-land policy may still realize as public open space inside the urban
/// zones: at most `ratio` of the unplanned urban land, and therefore at most `ratio` of the plan.
pub fn residual_landcover_m2(planned_m2: i64, ratio: f64) -> i64 {
    if !ratio.is_finite() || ratio <= 0. {
        return 0;
    }
    (planned_m2 as f64 * ratio.clamp(0.0, 1.0)).ceil() as i64
}

/// Urban land the plan already commits to roads and parcels, measured as an upper bound of the hard
/// physical cover the ledger will book (see the module documentation).
pub fn committed_urban_m2(plan: &RoadNetworkPlan, zones: &[LandUseZone]) -> i64 {
    let urban = urban_zone_rects(zones);
    let mut total = 0_i64;
    for entity in plan.entities() {
        total += clipped_m2(development::road_rect(&entity), &urban);
    }
    for site in &plan.roadside_sites {
        if let Ok(rect) = development::parcel_rect(site) {
            total += clipped_m2(rect, &urban);
        }
    }
    total
}

/// Urban land one roadside site's own frontage parcel commits.
pub fn site_urban_m2(site: &RoadsideSite, zones: &[LandUseZone]) -> i64 {
    let urban = urban_zone_rects(zones);
    development::parcel_rect(site).map_or(0, |rect| clipped_m2(rect, &urban))
}

/// The running account one plan spends conversion area against.
///
/// `committed_urban_m2` and `residual_landcover_m2` are measured ONCE, when the account is opened,
/// and never recomputed: a pad withdrawn later in planning can only lower the real commitment, so
/// keeping the larger opening figure keeps every later decision inside the smaller true allowance.
#[derive(Debug, Clone)]
pub struct UrbanAllocation {
    urban: Vec<Rect>,
    planned_m2: i64,
    committed_m2: i64,
    residual_m2: i64,
    converted_m2: i64,
    /// Conversion area held back for the authored pad programme. Deducted from what the demand-led
    /// frontage programme may spend, and spent by the time the pads are granted themselves.
    reserved_m2: i64,
    /// The reservation as PUBLISHED. Survives `resume`, so a record closed by the pad stage still
    /// reports what the frontage programme was held back from.
    published_reserved_m2: i64,
    refusals: Vec<ConversionRefusal>,
}

impl UrbanAllocation {
    /// Open the account on a plan whose roads and roadside pads are final.
    pub fn open(plan: &RoadNetworkPlan, zones: &[LandUseZone], residual_ratio: f64) -> Self {
        let urban = urban_zone_rects(zones);
        let planned_m2 = urban.iter().map(|zone| zone.area() as i64).sum();
        let committed_m2 = committed_urban_m2(plan, zones);
        let residual_m2 = residual_landcover_m2(planned_m2, residual_ratio);
        Self {
            urban,
            planned_m2,
            committed_m2,
            residual_m2,
            converted_m2: 0,
            reserved_m2: 0,
            published_reserved_m2: 0,
            refusals: Vec::new(),
        }
    }

    /// Continue an account another stage already opened, keeping its opening commitment and its
    /// refusal list. The reservation is spent by then: the pads it protected are granted next.
    pub fn resume(record: &ConversionAllocation, zones: &[LandUseZone]) -> Self {
        Self {
            urban: urban_zone_rects(zones),
            planned_m2: record.planned_urban_m2,
            committed_m2: record.committed_urban_m2,
            residual_m2: record.residual_landcover_m2,
            converted_m2: record.converted_m2,
            reserved_m2: 0,
            published_reserved_m2: record.reserved_for_pads_m2,
            refusals: record.refusals.clone(),
        }
    }

    /// Reserve conversion area for the authored pad programme before demand-led lots may spend it.
    pub fn reserve_for_pads(&mut self, area_m2: i64) {
        self.reserved_m2 += area_m2.max(0);
        self.published_reserved_m2 = self.reserved_m2;
    }

    pub fn planned_m2(&self) -> i64 {
        self.planned_m2
    }

    pub fn committed_m2(&self) -> i64 {
        self.committed_m2
    }

    pub fn converted_m2(&self) -> i64 {
        self.converted_m2
    }

    /// The conversion area this account can still grant.
    pub fn remaining_m2(&self) -> i64 {
        (self.planned_m2 - self.committed_m2 - self.residual_m2 - self.converted_m2 - self.reserved_m2)
            .max(0)
    }

    /// Whether `required_m2` still fits, without spending anything.
    pub fn fits(&self, required_m2: i64) -> bool {
        required_m2 >= 0 && required_m2 <= self.remaining_m2()
    }

    /// Urban land one site's own parcel commits, measured before the parcel lands.
    pub fn site_commitment(&self, site: &RoadsideSite) -> i64 {
        development::parcel_rect(site).map_or(0, |rect| clipped_m2(rect, &self.urban))
    }

    /// Refuse one candidate BY NAME, with the area it needed and the area that was left.
    ///
    /// The refusal keeps both numbers, so a reader can re-derive the decision from the published plan
    /// without re-running the planner.
    pub fn refuse(&mut self, parcel_id: &str, required_m2: i64) {
        let remaining_m2 = self.remaining_m2();
        self.refusals.push(ConversionRefusal {
            parcel_id: parcel_id.into(),
            required_m2,
            remaining_m2,
        });
    }

    /// Grant `required_m2` to `parcel_id`, or refuse it by name and record the evidence.
    pub fn grant(&mut self, parcel_id: &str, required_m2: i64) -> bool {
        if !self.fits(required_m2) {
            self.refuse(parcel_id, required_m2);
            return false;
        }
        self.spend(required_m2);
        true
    }

    /// Spend area a caller has already checked with [`Self::fits`].
    pub fn spend(&mut self, area_m2: i64) {
        self.converted_m2 += area_m2.max(0);
    }

    /// Account urban land a parcel that landed anyway commits.
    pub fn commit(&mut self, committed_m2: i64) {
        self.committed_m2 += committed_m2.max(0);
    }

    /// Take a parcel that landed anyway: its own frontage rectangle commits urban land too, and the
    /// allocation may not pretend that land is still free.
    pub fn commit_site(&mut self, site: &RoadsideSite) {
        let committed_m2 = self.site_commitment(site);
        self.commit(committed_m2);
    }

    pub fn refused(&self) -> &[ConversionRefusal] {
        &self.refusals
    }

    /// Close the account into the record the plan publishes. `None` when nothing was refused, so a
    /// world that never spent against the cap gains no key and stays byte-identical.
    pub fn close(self) -> Option<ConversionAllocation> {
        if self.refusals.is_empty() {
            return None;
        }
        Some(ConversionAllocation {
            planned_urban_m2: self.planned_m2,
            committed_urban_m2: self.committed_m2,
            residual_landcover_m2: self.residual_m2,
            converted_m2: self.converted_m2,
            reserved_for_pads_m2: self.published_reserved_m2,
            refusals: self.refusals,
        })
    }
}

/// Every conversion record the plan publishes, frontage programme first, then the roadside pads.
pub fn plan_conversion_records(plan: &RoadNetworkPlan) -> Vec<Conversion> {
    let mut records = plan
        .frontage
        .as_ref()
        .map(|report| report.conversions.clone())
        .unwrap_or_default();
    records.extend(plan.pad_conversions.iter().cloned());
    records
}

/// Everything an independent audit can re-derive about one plan's conversions from the plan and the
/// zoning alone. Named failures, never a count that happens to agree.
///
/// The check is the contract itself: the verified conversion area may not exceed the urban land the
/// plan allocated but did not commit, and each published refusal must name an area it could not
/// afford. A hand-edited plan that inflates a conversion therefore fails here by name instead of
/// inflating the urban row of the ledger.
pub fn allocation_failures(
    plan: &RoadNetworkPlan,
    zones: &[LandUseZone],
    converted_m2: i64,
) -> Vec<String> {
    let planned = planned_urban_m2(zones);
    let committed = committed_urban_m2(plan, zones);
    let mut failures = Vec::new();
    if planned <= 0 {
        failures.push(format!(
            "the plan converts {converted_m2} m2 of rural land to urban purpose but allocates no \
             urban land at all"
        ));
    }
    // The strongest bound the plan and the zoning alone can prove: the residual-land share is not
    // re-derivable here (it is a policy input), so the audited allowance is the one the plan states
    // when it published one, and `planned - committed` otherwise.
    let (residual, allowance) = match &plan.conversion_allocation {
        Some(allocation) => (
            allocation.residual_landcover_m2,
            allocation.allowance_m2(),
        ),
        None => (0, (planned - committed).max(0)),
    };
    if converted_m2 > allowance {
        failures.push(format!(
            "the plan converts {converted_m2} m2 of rural land to urban purpose, but its urban \
             allocation leaves only {allowance} m2 (planned {planned} m2 minus {committed} m2 \
             already committed to roads and parcels minus {residual} m2 of programmed public open \
             space)"
        ));
    }
    if let Some(allocation) = &plan.conversion_allocation {
        if allocation.planned_urban_m2 != planned {
            failures.push(format!(
                "the plan's conversion allocation claims {claimed} m2 of planned urban land but the \
                 zoning plans {planned} m2",
                claimed = allocation.planned_urban_m2
            ));
        }
        if allocation.committed_urban_m2 < committed {
            failures.push(format!(
                "the plan's conversion allocation was opened on {claimed} m2 of committed urban \
                 land, less than the {committed} m2 the plan commits",
                claimed = allocation.committed_urban_m2
            ));
        }
        if converted_m2 > allocation.converted_m2 {
            failures.push(format!(
                "the plan's conversion allocation granted {granted} m2 but its records convert \
                 {converted_m2} m2: a plan may keep less than it granted (a refused parcel takes its \
                 own record with it), never more",
                granted = allocation.converted_m2
            ));
        }
        if allocation.converted_m2 > allocation.allowance_m2() {
            failures.push(format!(
                "the plan's conversion allocation granted {granted} m2 out of an allowance of \
                 {allowance} m2, which its own numbers do not permit",
                granted = allocation.converted_m2,
                allowance = allocation.allowance_m2()
            ));
        }
        if allocation.reserved_for_pads_m2 < 0 || allocation.reserved_for_pads_m2 > planned {
            failures.push(format!(
                "the plan's conversion allocation reserves {reserved} m2 of the {planned} m2 of \
                 planned urban land for the pad programme",
                reserved = allocation.reserved_for_pads_m2
            ));
        }
        let mut refused = std::collections::BTreeSet::new();
        for refusal in &allocation.refusals {
            if !refused.insert(refusal.parcel_id.as_str()) {
                failures.push(format!(
                    "conversion parcel {} is refused twice",
                    refusal.parcel_id
                ));
            }
            if refusal.remaining_m2 < 0 || refusal.remaining_m2 > planned {
                failures.push(format!(
                    "conversion parcel {} was refused with {remaining} m2 left of a {planned} m2 \
                     allocation",
                    refusal.parcel_id,
                    remaining = refusal.remaining_m2
                ));
            }
            if refusal.required_m2 <= refusal.remaining_m2 {
                failures.push(format!(
                    "conversion parcel {} needed {required} m2 and was refused with {remaining} m2 \
                     left, which is not a refusal at all",
                    refusal.parcel_id,
                    required = refusal.required_m2,
                    remaining = refusal.remaining_m2
                ));
            }
        }
        for conversion in plan_conversion_records(plan) {
            if refused.contains(conversion.parcel_id.as_str()) {
                failures.push(format!(
                    "conversion parcel {} is refused by the allocation and still recorded as a \
                     conversion of {} m2",
                    conversion.parcel_id, conversion.area_m2
                ));
            }
        }
    }
    failures
}
