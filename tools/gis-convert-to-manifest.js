// Convert Overpass GIS JSON layers into a GlyphWeave manifest.
// WGS84 -> local metres via equirectangular projection centred on the bbox.
const fs = require('fs');
const path = require('path');
const GIS = 'G:/difyllmwiki/artifacts/gis';
const OUT_MANIFEST = 'G:/difyllmwiki/GlyphWeave/examples/shenzhen-bay-gis.manifest.json';

// Bbox centre (lat, lon) — the world origin.
const LAT0 = 22.500;
const LON0 = 113.939;
const M_PER_DEG_LAT = 111320.0;
const M_PER_DEG_LON = 111320.0 * Math.cos(LAT0 * Math.PI / 180.0);

function load(name) {
  return JSON.parse(fs.readFileSync(path.join(GIS, `szbay-${name}.json`), 'utf-8'));
}

// Build node coordinate lookup: id -> {x, z} in local metres.
function buildNodeMap(layers) {
  const map = new Map();
  for (const [name, j] of Object.entries(layers)) {
    for (const el of (j.elements || [])) {
      if (el.type !== 'node') continue;
      if (map.has(el.id)) continue;
      const dx = (el.lon - LON0) * M_PER_DEG_LON;
      const dz = (el.lat - LAT0) * M_PER_DEG_LAT;
      map.set(el.id, { x: dx, z: dz });
    }
  }
  return map;
}

// Compute way centroid + bounding box in local metres.
function wayGeometry(way, nodeMap) {
  const pts = (way.nodes || []).map(id => nodeMap.get(id)).filter(Boolean);
  if (pts.length < 2) return null;
  const xs = pts.map(p => p.x), zs = pts.map(p => p.z);
  const minX = Math.min(...xs), maxX = Math.max(...xs);
  const minZ = Math.min(...zs), maxZ = Math.max(...zs);
  const cx = (minX + maxX) / 2;
  const cz = (minZ + maxZ) / 2;
  return {
    cx, cz,
    w: maxX - minX,
    d: maxZ - minZ,
    pts,
  };
}

const layers = {
  buildings: load('buildings'),
  roads: load('roads'),
  water: load('water'),
  green: load('green'),
  bridge: load('bridge'),
};
const nodeMap = buildNodeMap(layers);

const entities = [];
const landmarks = [];
let buildingCount = 0, roadCount = 0;

// --- Buildings → EntityInstance -------------------------------------------
function inScene(x, z) {
  return x > 60 && x < 1940 && z > 60 && z < 1940;
}
let buildingIdx = 0;
for (const el of (layers.buildings.elements || [])) {
  if (el.type !== 'way' || !el.tags?.building) continue;
  // Deterministic sampling by index: keep ~1/3 for a dense real footprint.
  if (buildingIdx++ % 3 !== 0) continue;
  const g = wayGeometry(el, nodeMap);
  if (!g || g.w < 4 || g.d < 4 || g.w > 200 || g.d > 200) continue;
  const cx = g.cx + 1000, cz = g.cz + 1000;
  if (!inScene(cx, cz)) continue;
  // Whole footprint must sit inside the scene (validator checks half extents).
  if (cx - g.w / 2 < 30 || cx + g.w / 2 > 1970 || cz - g.d / 2 < 30 || cz + g.d / 2 > 1970) continue;
  // Height from OSM tags (levels) or deterministic.
  const levels = parseInt(el.tags['building:levels'], 10);
  const h = Math.max(3, isNaN(levels) ? 3 + (el.id % 5) : Math.round(levels * 3.2));
  const kind = ((el.tags.building === 'retail' || el.tags.building === 'commercial' || el.tags.shop)
      && g.w < 24 && g.d < 24)
    ? 'storefront' : (h > 30 ? 'building_tower' : 'building');
  entities.push({
    entityId: `gis.building.${el.id}`,
    assetId: `prop.${kind}`,
    kind,
    worldX: Math.round(cx), worldZ: Math.round(cz), worldY: 0,
    scale: 1.0,
    widthM: Math.round(g.w), depthM: Math.round(g.d), heightM: +h,
  });
  buildingCount++;
}

// --- Roads → road landmarks ------------------------------------------------
let roadIdx = 0;
for (const el of (layers.roads.elements || [])) {
  if (el.type !== 'way' || !el.tags?.highway) continue;
  if (roadIdx++ % 3 !== 0) continue;
  const g = wayGeometry(el, nodeMap);
  if (!g || g.w < 8 || g.d < 8) continue;
  const cx = g.cx + 1000, cz = g.cz + 1000;
  if (!inScene(cx, cz)) continue;
  const highway = el.tags.highway;
  const isMajor = ['trunk', 'primary', 'secondary'].includes(highway);
  const width = isMajor ? 12 : 6;
  const len = Math.max(g.w, g.d);
  // Road strips are axis-aligned; keep the whole footprint well inside the
  // 2000m scene so the validator's half-width check passes on both axes.
  if (len > 420) continue;
  if (cx - len / 2 < 30 || cx + len / 2 > 1970 || cz - len / 2 < 30 || cz + len / 2 > 1970) continue;
  entities.push({
    entityId: `gis.road.${el.id}`,
    assetId: 'prop.road', kind: 'road',
    worldX: Math.round(cx), worldZ: Math.round(cz), worldY: 1,
    scale: 1.0,
    widthM: Math.round(len), depthM: width, heightM: 1,
  });
  roadCount++;
}

// --- Water → river/lake landmarks -----------------------------------------
for (const el of (layers.water.elements || [])) {
  if (el.type !== 'way' || !(el.tags?.['natural'] === 'water' || el.tags?.waterway)) continue;
  const g = wayGeometry(el, nodeMap);
  if (!g) continue;
  // Keep the bay body and meaningful water features; drop tiny pond scraps.
  if (g.w < 40 && g.d < 40) continue;
  const cx = g.cx + 1000, cz = g.cz + 1000;
  // Water may straddle the scene edge (the bay is larger than the window);
  // keep anything that overlaps the visible area.
  if (cx < -300 || cx > 2300 || cz < -300 || cz > 2300) continue;
  const isSea = el.tags['natural'] === 'water' && g.w > 300;
  landmarks.push({
    entityId: `gis.water.${el.id}`, name: el.tags.name || (isSea ? '深圳湾' : '水体'),
    type: isSea ? 'river' : 'lake',
    purpose: el.tags.name || '水体',
    description: '来自 OpenStreetMap 的真实水体 footprint',
    sceneId: 'szbay-gis',
    worldX: Math.round(cx), worldZ: Math.round(cz), worldY: 0,
    widthM: Math.round(g.w), depthM: Math.round(g.d), heightM: 1,
    assetId: `water.${isSea ? 'szbay' : 'lake'}.${el.id}`,
  });
}

// --- Bridge → bridge landmark ---------------------------------------------
const bridgeWays = (layers.bridge.elements || []).filter(e => e.type === 'way' && e.tags?.bridge);
for (const el of bridgeWays.slice(0, 8)) {
  const g = wayGeometry(el, nodeMap);
  if (!g || g.w < 50 || g.d < 10) continue;
  const cx = g.cx + 1000, cz = g.cz + 1000;
  if (!inScene(cx, cz)) continue;
  const len = Math.max(g.w, g.d);
  if (len > 400 || cx - len / 2 < 30 || cx + len / 2 > 1970 || cz - len / 2 < 30 || cz + len / 2 > 1970) continue;
  landmarks.push({
    entityId: `gis.bridge.${el.id}`, name: el.tags.name || '跨海大桥', type: 'bridge',
    purpose: '真实 OSM 桥梁', description: '来自 OpenStreetMap 的桥梁 footprint',
    sceneId: 'szbay-gis',
    worldX: Math.round(cx), worldZ: Math.round(cz), worldY: 5,
    widthM: Math.round(len), depthM: Math.max(5, Math.round(g.d * 0.05)), heightM: 20,
    assetId: `bridge.gis.${el.id}`,
  });
}

// --- Entities → landmarks (buildings + roads) -----------------------------
// The manifest carries landmarks; generate_entities_template turns them into
// EntityInstances verbatim, so GIS buildings/roads become landmarks too.
const sceneId = 'szbay-gis';
for (const e of entities) {
  landmarks.push({
    entityId: e.entityId, name: e.kind,
    type: e.kind,
    purpose: e.kind,
    description: '来自 OpenStreetMap 的真实 footprint',
    sceneId,
    worldX: e.worldX, worldZ: e.worldZ, worldY: e.worldY,
    widthM: e.widthM, depthM: e.depthM, heightM: e.heightM,
    assetId: e.assetId,
  });
}

// --- Manifest --------------------------------------------------------------
const manifest = {
  format: 'glyphweave-world',
  version: 1,
  world: { name: '深圳湾（真实 GIS 数据）', seed: 20260830, renderMode: '2.5d' },
  scenes: [{
    sceneId: 'szbay-gis',
    widthM: 2000, depthM: 2000,
    originX: 0, originZ: 0,
    seedOffset: 0,
  }],
  style: {
    family: 'procedural-modern-coastal-city',
    terrain: 'continuous-heightfield',
    layoutReference: 'Real Shenzhen Bay footprints from OpenStreetMap (Overpass API), centred on 22.5N 113.945E.',
    referenceData: {
      sources: ['https://www.openstreetmap.org/'],
      retrievedAt: '2026-08-30',
      api: 'Overpass API',
      bbox: '22.485,113.930,22.515,113.970',
      inferenceReason: 'Real building footprints / roads / water polygons from OSM, converted to local metres.',
    },
    water: {
      waterType: 'river', levelPolicy: 'horizontal-datum', levelM: 0,
      flowDirection: 'world_z', flowSpeedMps: 0.3,
      shoreProfile: { transitionWidthM: 40, shallowBandM: 20, navigationDepthM: 6 },
      waveModel: { preset: 'harbour-wind', amplitudeM: 0.08, wavelengthM: 14, direction: 'world_z', roughness: 0.2 },
    },
    assetContracts: {
      river: { type: 'water', placement: 'water-surface', allowedSurfaces: ['water'], forbiddenSurfaces: ['underground'], minWidthM: 20, maxWidthM: 2500, minDepthM: 20, maxDepthM: 2500, minHeightM: 1, maxHeightM: 1 },
      road: { type: 'road', placement: 'surface-grounded', allowedSurfaces: ['grass', 'soil', 'stone'], forbiddenSurfaces: ['water', 'underground'], minWidthM: 8, maxWidthM: 2500, minDepthM: 4, maxDepthM: 20, minHeightM: 0.1, maxHeightM: 1 },
      bridge: { type: 'bridge', placement: 'bridge-deck', allowedSurfaces: ['water', 'shore'], forbiddenSurfaces: ['underground'], minWidthM: 10, maxWidthM: 800, minDepthM: 5, maxDepthM: 40, minHeightM: 5, maxHeightM: 60 },
      building: { type: 'building', placement: 'surface-grounded', allowedSurfaces: ['grass', 'soil', 'stone'], forbiddenSurfaces: ['water', 'underground'], minWidthM: 4, maxWidthM: 200, minDepthM: 4, maxDepthM: 200, minHeightM: 3, maxHeightM: 120 },
      building_tower: { type: 'building', placement: 'surface-grounded', allowedSurfaces: ['grass', 'soil', 'stone'], forbiddenSurfaces: ['water', 'underground'], minWidthM: 10, maxWidthM: 200, minDepthM: 10, maxDepthM: 200, minHeightM: 20, maxHeightM: 150 },
      storefront: { type: 'street-front', placement: 'surface-grounded', allowedSurfaces: ['grass', 'soil', 'stone'], forbiddenSurfaces: ['water', 'underground'], minWidthM: 4, maxWidthM: 100, minDepthM: 4, maxDepthM: 100, minHeightM: 3, maxHeightM: 40 },
      resort_lodge: { type: 'building', placement: 'surface-grounded', allowedSurfaces: ['grass', 'soil', 'stone', 'shore'], forbiddenSurfaces: ['water', 'deep_water', 'underground'], minWidthM: 16, maxWidthM: 120, minDepthM: 12, maxDepthM: 120, minHeightM: 4, maxHeightM: 40 },
      lake: { type: 'water', placement: 'water-surface', allowedSurfaces: ['water'], forbiddenSurfaces: ['underground'], minWidthM: 20, maxWidthM: 2500, minDepthM: 20, maxDepthM: 2500, minHeightM: 1, maxHeightM: 1 },
      sidewalk: { type: 'pedestrian-pavement', placement: 'surface-grounded', allowedSurfaces: ['stone', 'shore', 'soil'], forbiddenSurfaces: ['water', 'underground'], minWidthM: 2, maxWidthM: 2000, minDepthM: 2, maxDepthM: 8, minHeightM: 0.1, maxHeightM: 1 },
      pedestrian: { type: 'pedestrian', placement: 'sidewalk-surface', allowedSurfaces: ['stone', 'shore', 'soil'], forbiddenSurfaces: ['water', 'underground'], minWidthM: 0.4, maxWidthM: 1.2, minDepthM: 0.4, maxDepthM: 1.2, minHeightM: 1.4, maxHeightM: 2.2 },
      car: { type: 'vehicle', placement: 'road-surface', allowedSurfaces: ['road', 'bridge'], forbiddenSurfaces: ['water', 'underground'], minWidthM: 4, maxWidthM: 6, minDepthM: 1.6, maxDepthM: 2.4, minHeightM: 1.2, maxHeightM: 2 },
      tree: { type: 'tree', placement: 'surface-grounded', allowedSurfaces: ['grass', 'forest', 'soil', 'shore'], forbiddenSurfaces: ['water', 'underground'], minWidthM: 2, maxWidthM: 5.5, minDepthM: 2, maxDepthM: 5.5, minHeightM: 4, maxHeightM: 9 },
      bush: { type: 'bush', placement: 'surface-grounded', allowedSurfaces: ['grass', 'forest', 'soil', 'shore'], forbiddenSurfaces: ['water', 'underground'], minWidthM: 0.5, maxWidthM: 3, minDepthM: 0.5, maxDepthM: 3, minHeightM: 0.3, maxHeightM: 3 },
      rock: { type: 'rock', placement: 'surface-grounded', allowedSurfaces: ['grass', 'soil', 'stone', 'shore'], forbiddenSurfaces: ['water', 'underground'], minWidthM: 0.2, maxWidthM: 4, minDepthM: 0.2, maxDepthM: 4, minHeightM: 0.1, maxHeightM: 4 },
      lamp: { type: 'street-furniture', placement: 'surface-grounded', allowedSurfaces: ['grass', 'shore', 'stone', 'soil'], forbiddenSurfaces: ['water', 'underground'], minWidthM: 0.2, maxWidthM: 1, minDepthM: 0.2, maxDepthM: 1, minHeightM: 2, maxHeightM: 7 },
      grass_clump: { type: 'grass', placement: 'surface-grounded', allowedSurfaces: ['grass', 'forest', 'soil', 'shore'], forbiddenSurfaces: ['water', 'underground'], minWidthM: 0.5, maxWidthM: 3, minDepthM: 0.5, maxDepthM: 3, minHeightM: 0.2, maxHeightM: 4 },
      bench: { type: 'street-furniture', placement: 'surface-grounded', allowedSurfaces: ['grass', 'shore', 'stone', 'soil'], forbiddenSurfaces: ['water', 'underground'], minWidthM: 2, maxWidthM: 5, minDepthM: 0.5, maxDepthM: 2, minHeightM: 0.4, maxHeightM: 2 },
      fallen_log: { type: 'fallen-log', placement: 'surface-grounded', allowedSurfaces: ['grass', 'forest', 'soil', 'shore'], forbiddenSurfaces: ['water', 'underground'], minWidthM: 1, maxWidthM: 6, minDepthM: 0.3, maxDepthM: 2, minHeightM: 0.2, maxHeightM: 2 },
      reed: { type: 'reed', placement: 'surface-grounded', allowedSurfaces: ['shore', 'wetland'], forbiddenSurfaces: ['deep_water', 'underground'], minWidthM: 0.2, maxWidthM: 1, minDepthM: 0.2, maxDepthM: 1, minHeightM: 1, maxHeightM: 5 },
    },
  },
  landmarks,
  sceneGraph: { transitions: [] },
};

fs.mkdirSync(path.dirname(OUT_MANIFEST), { recursive: true });
fs.writeFileSync(OUT_MANIFEST, JSON.stringify(manifest, null, 2));
console.log(`manifest written: ${OUT_MANIFEST}`);
console.log(`buildings=${buildingCount} roads=${roadCount} waterLandmarks=${landmarks.filter(l=>l.type!=='bridge').length} bridges=${landmarks.filter(l=>l.type==='bridge').length}`);
