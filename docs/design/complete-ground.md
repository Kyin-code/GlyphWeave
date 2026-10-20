# Complete-ground surface coverage (B-03)

Status: implemented as an opt-in policy on 2026-10-20.

## Problem

The land-use ledger has four purpose rows (urban, rural, nature, unassigned) and
physical-surface columns. Before this change, agriculture, yards, public open
space and conservation were real meshes, but the rest of a rural or urban cell
had no surface at all. The ledger therefore reported a large `unresolvedM2`
even when every authored programme had succeeded. At 6x10 km the gap was
50,538,180 m2; at the 512 m B-03 control it was 128,542 m2.

The missing fact was not "more buildings". It was "what surface covers the
remaining authored land".

## Policy

```json
"residualLandProgram": {
  "version": 1,
  "scenes": {
    "scene-0": {
      "urbanOpenSpaceRatio": 0.1,
      "ruralPastureRatio": 0.2,
      "coverage": "complete-ground-v1"
    }
  }
}
```

`coverage` is absent by default. Old worlds keep their bytes and their
`unresolved` values. Unknown coverage strings are refused.

The named ratios remain managed sub-use policy. They are not silently expanded
to 100%. `complete-ground-v1` is a separate statement that the *rest* of the
non-yard source cell has a real background surface.

## Realization

The pass runs after yards, shoreline band, agriculture, conservation and the
named residual sub-uses:

1. Rasterize every existing accepted mesh into a one-bit-per-metre occupancy
   map.
2. Walk each non-yard source cell row by row. Uncovered 1 m cells become
   horizontal runs, separated at water boundaries.
3. Merge identical runs vertically, then emit terrain-conforming quads of at
   most 32 m x 32 m.
4. Material comes from the final terrain and the authored water body:

   - `urban_vacant_ground` for urban remainder;
   - `rural_meadow`, `rural_woodland_floor`, `rural_rough_ground` for rural
     grade bands;
   - `unassigned_ground` for unassigned remainder;
   - `water_surface` for the terrain bed under authored water.

The occupancy map is the anti-double-claim rule. A baseline quad may not
overlap an accepted yard, field, headland, track, open space or conservation
patch. Managed yards are excluded because a yard is a usable-space claim, not
a background ground claim; its own realization/rejection rules still apply.

## Accounting

- `PhysicalCover.groundCoverM2` and `waterSurfaceM2` are additive optional
  columns. Old ledgers do not serialize them.
- The residual audit counts baseline materials as realization of the cell's
  purpose. `policyRemainderM2` remains a policy statement and is not a second
  deficit.
- `landUseBudget.actualNatureM2` is now measured from landcover surfaces
  instead of being hard-coded to zero.
- `developmentAudit` retires
  `remaining_land_has_no_development_or_conservation_plan` only when the
  independent surface audit has zero unresolved area.

## Limits

This is a coarse surface layer, not a biome simulation. It is deliberately
designed to accept a later landform-conditioned ecology/habitat provider:
slope, elevation, water distance and landform class can replace the current
grade-only classification without changing the ledger contract.

Water is represented as the bed under the authored water body; the visible
water surface remains the existing water renderer. This avoids drawing a
second water mesh at the same height.
