# Engineering works: retaining walls, culverts and drainage feedback

Status: first C-E1/C-E2 implementation, 2026-10-20.

## Policy

```json
"terrainWorks": {
  "retainingWalls": "auto",
  "drainageStructures": "auto",
  "minFillMm": 300,
  "maxWallHeightMm": 6000,
  "maxStructures": 4096
},
"engineeringDrainage": "audit"
```

Absent policy is a no-op. The old snapshot keeps its bytes. Unknown values,
out-of-range heights and structure-budget overflow are named failures.

## Retaining walls

Walls are derived from the `EngineeredTerrainOperation`s that were actually
applied to the final raster. For each operation, the difference between final
platform height and natural ground at the centre is measured:

- thinner than `minFillMm`: no wall;
- between the minimum and `maxWallHeightMm`: a `retaining_wall` record with two
  stable endpoints, thickness, face area and volume;
- above the cap: a named failure, no short wall invented.

Each wall carries:

`id`, `kind`, `entityId`, `sourceOperationId`, `start`, `end`, `heightM`,
`lengthM`, `thicknessM`, `volumeM3`, `material`, `status`.

The record is a renderable data primitive (a consumer can extrude the segment)
but the Rust core does not require a 3-D mesh. This keeps renderer work
replaceable while making the engineering fact authoritative.

## Culverts

`drainageStructures: "auto"` turns each registered road/water crossing into a
`culvert` record:

`id`, `roadId`, `waterId`, `span`, `lengthM`, `wetCells`, `invertMm`, `deckMm`,
`diameterM`, `status`.

The record is tied to the existing `WaterCrossing`; the water surface itself is
never rewritten.

## Drainage feedback

`engineeringDrainage: "audit"` re-derives the D8 sampled routing graph from the
final heightfield and compares it with the natural graph node by node:

- changed flow destinations;
- new and lost terminal sinks;
- terminals raised by more than 50 mm;
- integrity failures of either graph.

This is explicitly `sampled-routing-integrity-not-hydraulic-safety`. It answers
"where did engineering reroute the sampled flow?"; it does not claim flood
capacity or discharge.

## Current limits

- Walls are placed on the operation perimeter using the centre fill depth. A
  later pass should choose only the downhill faces and vary height along the
  edge.
- The culvert record is a metadata/geometry contract; hydraulic capacity is a
  later model.
- The drainage comparison is at 32 m sampling, not 1 m hydraulics.
