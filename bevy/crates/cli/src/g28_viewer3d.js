/* G28 saved-scene context preview 3D viewer.
 *
 * Read-only fact viewer. It builds no EngineeringInput, computes no route,
 * certifies no terrain and never promotes an unverified access record to a
 * route. Honesty rules kept from G27B and enforced by the G28 tests:
 *   - heights are the saved integer millimetres; metres exist only as a
 *     lossless display conversion, and no height is ever invented;
 *   - nodes and access records carry only saved x/z, so they are never given a
 *     world Y and never drawn as 3D primitives; they appear only in the
 *     explicitly labelled 2D observation overlay and in the HUD counts;
 *   - access records stay sourceObservationOnly / notReverified;
 *   - picking is real indexed-triangle intersection against the drawn geometry;
 *     a derived hit point is display geometry and is never written back as a
 *     saved millimetre fact.
 *
 * G28 additions: a selection panel fed by the same object as the highlight and
 * window.__PREVIEW3D_STATE__; split HUD counts per category; camera presets and
 * pointer rotate/zoom; a collapsible HUD; layered materials; flat-normal
 * lighting with a mid-tone background and depth fog.
 */
(function () {
  "use strict";

  // ------------------------------------------------------------------ config
  // Contrast pass after the three-view review: the first G28 values rendered the
  // G31 candidate (g31-render-readability-1): display parameters only. The ground
  // was being washed by the fog (19% at the patch centre, 43% at the far rim), the
  // brightest terrain samples clipped above 1.0, the entity top/side ratio was 1.66
  // and the distant road converged on the same grey as the terrain. Background, fog
  // and the ramp ends move together so an up-facing terrain keeps a clear value
  // separation from the background while no highlight is clipped, and the ambient
  // share drops so the directional light carries the block faces. Pre-declared in
  // visual-intent.zh-CN.md before any value was changed.
  //
  // G31-R1 entity layering (g31-r1-entity-layering-1): the G31 entity probe sampled
  // its whole ROI box, background included, so its C4 number never described the
  // block. Re-measured on the saved shots through the projected entity geometry the
  // G31 candidate reached only 1.0839x against the 1.10x C4 threshold. The one
  // authorized revision therefore trades ambient for fill along the constant
  // up-face factor line (delta FILL_WEIGHT = -delta AMBIENT / 0.3436, 0.3436 being
  // the up-face coefficient of FILL_DIR): AMBIENT 0.38 -> 0.36 with FILL_WEIGHT
  // 0.18 -> 0.238 leaves every up-facing surface (terrain, roads, entity roof) at
  // exactly its G31 brightness, so the accepted terrain/background contrast, the
  // clipping budget and the top view are unchanged, while the two visible entity
  // side faces (which get no fill) darken and the entity top/side layering improves.
  // Pre-declared in the G31-R1 visual intent and c4-spec.json before the revision
  // package was built; the light directions, the shader, the selection colours, the
  // materials, the camera, the geometry and the model are untouched.
  var BACKGROUND = [0.15, 0.18, 0.24];
  var FOG_COLOUR = [0.26, 0.30, 0.37];
  var LIGHT_DIR = [0.45, 0.80, 0.40];
  var FILL_DIR = [-0.50, 0.30, -0.65];
  var AMBIENT = 0.36;
  var KEY_WEIGHT = 0.66;
  var FILL_WEIGHT = 0.238;
  var SKY_WEIGHT = 0.18;
  var FOG_STRENGTH = 0.42;
  var FOG_NEAR_FACTOR = 1.05;
  var FOG_FAR_FACTOR = 3.20;
  var MATERIALS = {
    final: {
      label: "final (saved surface layer)",
      source: "crop.finalMm",
      ramp: [[0.40, 0.60, 0.48], [0.58, 0.72, 0.50], [0.85, 0.85, 0.72]]
    },
    natural: {
      label: "natural (independent saved layer)",
      source: "crop.naturalMm",
      ramp: [[0.56, 0.46, 0.38], [0.70, 0.60, 0.46], [0.85, 0.78, 0.62]]
    },
    road: [0.52, 0.55, 0.62],
    roadSelected: [0.95, 0.74, 0.32],
    entity: [0.78, 0.74, 0.66],
    entitySelected: [0.95, 0.74, 0.32]
  };
  var OBSERVATION_LABEL = "2D observation overlay - saved x/z only; no world Y, no snapping, no route verification";
  // G30 reading aids. The 2D overlay colours live here so the legend and the
  // drawing code read the same values; no colour value changed in this round.
  var OBSERVATION_COLOURS = { nodes: "rgba(150,205,255,0.95)", access: "rgba(240,184,73,0.95)", accessFar: "rgba(240,184,73,0.42)" };
  // Local world axes of the saved scene. X is east, Y is up and Z is south in
  // this local frame; it is not a geodetic north and the report says so.
  var WORLDAXES = [
    { key: "east", label: "东 X+", short: "东", vector: [1, 0, 0], colour: "#8fb0d8" },
    { key: "west", label: "西 X-", short: "西", vector: [-1, 0, 0], colour: "#8fb0d8" },
    { key: "south", label: "南 Z+", short: "南", vector: [0, 0, 1], colour: "#9fc7a8" },
    { key: "north", label: "北 Z-", short: "北", vector: [0, 0, -1], colour: "#9fc7a8" },
    { key: "up", label: "上 Y+", short: "上", vector: [0, 1, 0], colour: "#c9c2a6" }
  ];
  var AXIS_DEGENERATE_LENGTH = 0.08;

  var VERTEX_SRC =
    "#version 300 es\n" +
    "precision highp float;\n" +
    "in vec3 aPos;\n" +
    "in vec3 aColor;\n" +
    "uniform mat4 uMvp;\n" +
    "out vec3 vColor;\n" +
    "out vec3 vWorld;\n" +
    "void main() {\n" +
    "  vColor = aColor;\n" +
    "  vWorld = aPos;\n" +
    "  gl_Position = uMvp * vec4(aPos, 1.0);\n" +
    "}\n";
  var FRAGMENT_SRC =
    "#version 300 es\n" +
    "precision highp float;\n" +
    "in vec3 vColor;\n" +
    "in vec3 vWorld;\n" +
    "out vec4 fragColor;\n" +
    "uniform vec3 uLightDir;\n" +
    "uniform vec3 uFillDir;\n" +
    "uniform vec3 uEye;\n" +
    "uniform vec3 uFogColour;\n" +
    "uniform vec2 uFogRange;\n" +
    "uniform float uAmbient;\n" +
    "void main() {\n" +
    "  vec3 normal = normalize(cross(dFdx(vWorld), dFdy(vWorld)));\n" +
    "  if (dot(normal, normalize(uEye - vWorld)) < 0.0) { normal = -normal; }\n" +
    "  float key = max(dot(normal, normalize(uLightDir)), 0.0);\n" +
    "  float fill = max(dot(normal, normalize(uFillDir)), 0.0);\n" +
    "  float sky = 0.5 + 0.5 * normal.y;\n" +
    "  vec3 lit = vColor * (uAmbient + " + KEY_WEIGHT.toFixed(3) + " * key + " + FILL_WEIGHT.toFixed(3) + " * fill + " + SKY_WEIGHT.toFixed(3) + " * sky);\n" +
    "  float depth = distance(vWorld, uEye);\n" +
    "  float fog = clamp((depth - uFogRange.x) / max(uFogRange.y - uFogRange.x, 0.001), 0.0, 1.0);\n" +
    "  fragColor = vec4(mix(lit, uFogColour, fog * " + FOG_STRENGTH.toFixed(3) + "), 1.0);\n" +
    "}\n";

  var state = {
    modelLoaded: false, errorMessage: null, isWebGL2: false, glVersion: null, glRenderer: null,
    surface: "final", surfaceSource: null, surfaceMaterial: null, vertexCount: 0, triangleCount: 0,
    surfaceVertexCount: 0, terrainIndexCount: 0, terrainTriangleCount: 0,
    modelDeclaredTerrainTriangles: 0, modelDeclaredTerrainVertices: 0,
    terrainIndicesBySurface: {}, terrainTrianglesBySurface: {},
    entityRoadTriangleCount: 0, submittedIndices: 0, indexedDrawCalls: 0, solidDrawCalls: 0,
    terrainDrawnWithIndexBuffer: false, terrainTopologyValidated: false,
    entityCount: 0, entitySolidCount: 0, roadCount: 0, roadSolidCount: 0,
    nodeCount: 0, accessPathCount: 0,
    entitySolidsDrawn: 0, roadSolidsDrawn: 0, drawnTriangleCount: 0, surfaceTriangleCount: 0,
    observation2dObjectCount: 0, observation2dNodes: 0, observation2dAccess: 0,
    heightlessObjectCount: 0, heightlessByCategory: null,
    notPositionedCount: 0, notPositionedByCategory: null,
    notSnappedCount: 0, notSnappedByCategory: null, snappedCount: 0,
    constantWorldYUsed: false, snapping: "none",
    visibleLayers: [], hiddenLayers: [],
    camera: { mode: "oblique", yaw: 0.6, pitch: 0.7, distance: 0, target: [0, 0, 0], minDistance: 0, maxDistance: 0 },
    cameraMoves: 0, cameraPresetLog: [],
    selection: null, selectedEntityId: null, selectedFacts: null, terrainHit: null, roadHit: null,
    selectionPanelText: null, selectionIsSameObjectAsHighlight: null,
    pickMethod: null, pickObject: null, pickCount: 0, pickMisses: 0,
    overlayVisible: true, overlayRecordsDrawn: 0, overlayLabel: OBSERVATION_LABEL,
    hudCollapsed: false, hud: null, hudRefreshes: 0,
    reading: null, readingRefreshes: 0,
    // G30-R3: the compact summary is always on; the full reading details live behind
    // the 读图详情 entry point in the inspect sidebar.
    readingDetailsOpen: false, readingDetailSignature: null,
    // G29 display-mode state: one model and one scene, two information layouts.
    viewMode: "browse", modeSwitches: 0, layoutSyncs: 0, layout: null,
    selectionDetailsOpen: false, pageLoads: 0,
    // G32 navigation state: published so a test can tell a pan from a rotate without
    // reading pixels, and so a rejected (non-finite) pan is visible instead of silent.
    // G32-R1: panCount counts COMPLETED pan gestures, panMoves counts APPLIED pan steps,
    // panRejections counts rejected pan GESTURES (at most one per gesture).
    panCount: 0, panMoves: 0, lastPan: null, panRejections: 0, lastPanRejection: null,
    pointerGesture: null, gestureLog: [],
    // G33 navigation overview minimap. It keeps its OWN drag state so the G32
    // pointer gesture contract (state.pointerGesture) is not touched, and it publishes
    // a read-only snapshot so a test can tell an overview jump from a map gesture.
    minimap: null, minimapDrag: null, minimapDragLog: [], minimapMoves: 0, minimapJumps: 0, lastMinimap: null,
    loadMs: null, frameMs: null, renderCount: 0, renderedPixelVariety: 0,
    lighting: null, background: null, fog: null, drawCalls: 0
  };
  window.__PREVIEW3D_STATE__ = state;

  var model = null, gl = null, program = null, buffers = {}, uniforms = {};
  var selectedId = null, drag = null;

  // ------------------------------------------------------------------ helpers
  function clamp(value, low, high) { return value < low ? low : (value > high ? high : value); }
  function round6(value) { return Math.round(value * 1e6) / 1e6; }
  function escapeHtml(value) {
    return String(value === null || value === undefined ? "" : value)
      .replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
  }
  function sub3(a, b) { return [a[0] - b[0], a[1] - b[1], a[2] - b[2]]; }
  function cross3(a, b) { return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]; }
  function dot3(a, b) { return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]; }
  function norm3(v) { var l = Math.hypot(v[0], v[1], v[2]) || 1; return [v[0] / l, v[1] / l, v[2] / l]; }

  function fail(message) {
    state.errorMessage = message;
    state.modelLoaded = false;
    model = null;
    state.selection = null;
    state.selectedEntityId = null;
    state.selectedFacts = null;
    state.terrainHit = null;
    state.roadHit = null;
    state.overlayRecordsDrawn = 0;
    selectedId = null;
    var statusLabel = document.getElementById("load-status");
    if (statusLabel) { statusLabel.textContent = "failed"; statusLabel.className = "err"; }
    var error = document.getElementById("load-error");
    if (error) { error.textContent = message; }
    renderSelection();
    drawObservationOverlay();
    if (gl) { gl.clearColor(BACKGROUND[0], BACKGROUND[1], BACKGROUND[2], 1); gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT); }
  }

  function compile(type, source) {
    var shader = gl.createShader(type);
    gl.shaderSource(shader, source);
    gl.compileShader(shader);
    if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) { throw new Error(gl.getShaderInfoLog(shader)); }
    return shader;
  }

  function resizeCanvas() {
    var canvas = document.getElementById("gl");
    var stage = document.getElementById("stage");
    var width = Math.max(1, Math.floor(stage ? stage.clientWidth : canvas.clientWidth || 900));
    var height = Math.max(1, Math.floor(stage ? stage.clientHeight : canvas.clientHeight || 700));
    if (canvas.width !== width || canvas.height !== height) { canvas.width = width; canvas.height = height; }
    var overlay = document.getElementById("overlay");
    if (overlay) {
      var rect = overlay.getBoundingClientRect();
      var ow = Math.max(1, Math.floor(rect.width || width));
      var oh = Math.max(1, Math.floor(rect.height || height));
      if (overlay.width !== ow || overlay.height !== oh) { overlay.width = ow; overlay.height = oh; }
    }
    return canvas;
  }

  function initGl() {
    var canvas = resizeCanvas();
    gl = canvas.getContext("webgl2", { antialias: true });
    if (!gl) { throw new Error("WebGL2 context is unavailable"); }
    state.isWebGL2 = typeof WebGL2RenderingContext !== "undefined" && gl instanceof WebGL2RenderingContext;
    state.glVersion = gl.getParameter(gl.VERSION);
    var dbg = gl.getExtension("WEBGL_debug_renderer_info");
    state.glRenderer = dbg ? gl.getParameter(dbg.UNMASKED_RENDERER_WEBGL) : "unavailable";
    program = gl.createProgram();
    gl.attachShader(program, compile(gl.VERTEX_SHADER, VERTEX_SRC));
    gl.attachShader(program, compile(gl.FRAGMENT_SHADER, FRAGMENT_SRC));
    gl.linkProgram(program);
    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) { throw new Error(gl.getProgramInfoLog(program)); }
    gl.useProgram(program);
    gl.enable(gl.DEPTH_TEST);
    gl.clearColor(BACKGROUND[0], BACKGROUND[1], BACKGROUND[2], 1);
    uniforms = {
      mvp: gl.getUniformLocation(program, "uMvp"),
      lightDir: gl.getUniformLocation(program, "uLightDir"),
      fillDir: gl.getUniformLocation(program, "uFillDir"),
      eye: gl.getUniformLocation(program, "uEye"),
      fogColour: gl.getUniformLocation(program, "uFogColour"),
      fogRange: gl.getUniformLocation(program, "uFogRange"),
      ambient: gl.getUniformLocation(program, "uAmbient")
    };
    gl.uniform3fv(uniforms.lightDir, new Float32Array(LIGHT_DIR));
    gl.uniform3fv(uniforms.fillDir, new Float32Array(FILL_DIR));
    gl.uniform3fv(uniforms.fogColour, new Float32Array(FOG_COLOUR));
    gl.uniform1f(uniforms.ambient, AMBIENT);
    state.lighting = { model: "flat face normal from screen-space derivatives, key + fill + sky terms", keyDirection: LIGHT_DIR, fillDirection: FILL_DIR, ambient: AMBIENT };
    state.background = { colour: BACKGROUND, note: "single mid-tone background; deliberately not pure black so uncovered area stays readable" };
    state.fog = { colour: FOG_COLOUR, nearFactor: FOG_NEAR_FACTOR, farFactor: FOG_FAR_FACTOR, note: "linear depth fog blended into the background colour" };
  }

  function mat4Multiply(a, b) {
    var out = new Float32Array(16);
    for (var i = 0; i < 4; i++) for (var j = 0; j < 4; j++) {
      var sum = 0; for (var k = 0; k < 4; k++) { sum += a[k * 4 + j] * b[i * 4 + k]; }
      out[i * 4 + j] = sum;
    }
    return out;
  }
  function perspective(fov, aspect, near, far) {
    var f = 1 / Math.tan(fov / 2), nf = 1 / (near - far);
    return new Float32Array([f / aspect, 0, 0, 0, 0, f, 0, 0, 0, 0, (far + near) * nf, -1, 0, 0, 2 * far * near * nf, 0]);
  }
  function lookAt(eye, centre, up) {
    var z = [eye[0] - centre[0], eye[1] - centre[1], eye[2] - centre[2]];
    var zl = Math.hypot(z[0], z[1], z[2]) || 1; z = [z[0] / zl, z[1] / zl, z[2] / zl];
    var x = [up[1] * z[2] - up[2] * z[1], up[2] * z[0] - up[0] * z[2], up[0] * z[1] - up[1] * z[0]];
    var xl = Math.hypot(x[0], x[1], x[2]) || 1; x = [x[0] / xl, x[1] / xl, x[2] / xl];
    var y = [z[1] * x[2] - z[2] * x[1], z[2] * x[0] - z[0] * x[2], z[0] * x[1] - z[1] * x[0]];
    return new Float32Array([x[0], y[0], z[0], 0, x[1], y[1], z[1], 0, x[2], y[2], z[2], 0,
      -(x[0] * eye[0] + x[1] * eye[1] + x[2] * eye[2]),
      -(y[0] * eye[0] + y[1] * eye[1] + y[2] * eye[2]),
      -(z[0] * eye[0] + z[1] * eye[1] + z[2] * eye[2]), 1]);
  }
  function cameraEye() {
    var c = state.camera;
    return [
      c.target[0] + c.distance * Math.cos(c.pitch) * Math.sin(c.yaw),
      c.target[1] + c.distance * Math.sin(c.pitch),
      c.target[2] + c.distance * Math.cos(c.pitch) * Math.cos(c.yaw)
    ];
  }

  // ------------------------------------------------------------------ camera
  function windowCentre() {
    var w = model.window;
    return [w.minX + w.widthM / 2, 0, w.minZ + w.depthM / 2];
  }
  function cameraLimits() {
    var w = model.window, span = Math.max(w.widthM, w.depthM);
    return { min: span * 0.12, max: span * 6.0, span: span };
  }
  function applyCameraPreset(preset) {
    if (!model) { return null; }
    var limits = cameraLimits(), c = state.camera;
    if (preset === "default" || preset === "oblique") {
      c.mode = "oblique"; c.yaw = 0.6; c.pitch = 0.7; c.distance = limits.span * 1.4; c.target = windowCentre();
    } else if (preset === "top") {
      c.mode = "top"; c.yaw = 0.0; c.pitch = 1.5533; c.distance = limits.span * 1.55; c.target = windowCentre();
    } else if (preset === "focus") {
      var anchor = (state.selection && state.selection.facts) ? state.selection.facts : null;
      if (anchor && typeof anchor.worldX === "number" && typeof anchor.worldZ === "number") {
        var height = typeof anchor.heightM === "number" ? anchor.heightM : 0;
        var size = Math.max(anchor.widthM || 1, anchor.depthM || 1, height || 1);
        c.mode = "focus"; c.target = [anchor.worldX, (anchor.worldY || 0) + height / 2, anchor.worldZ];
        c.distance = Math.max(size * 3.2, limits.span * 0.12);
      } else {
        c.mode = "focus-window"; c.target = windowCentre(); c.distance = limits.span * 0.45;
      }
    }
    c.minDistance = limits.min;
    c.maxDistance = limits.max;
    state.cameraMoves++;
    state.cameraPresetLog.push({ preset: preset, mode: c.mode });
    render();
    return cameraSnapshot();
  }
  function cameraSnapshot() {
    var c = state.camera;
    return {
      mode: c.mode, yaw: round6(c.yaw), pitch: round6(c.pitch), distance: round6(c.distance),
      target: [round6(c.target[0]), round6(c.target[1]), round6(c.target[2])]
    };
  }
  function rotateCamera(dx, dy) {
    var c = state.camera;
    c.yaw = c.yaw - dx * 0.006;
    c.pitch = clamp(c.pitch + dy * 0.005, -1.45, 1.5533);
    c.mode = "free";
    state.cameraMoves++;
    render();
  }
  // ------------------------------------------------------------------ G32 pan
  // Shift+left drag pans the horizontal target plane. The whole contract (signs,
  // scale, threshold, lock and cancel rules) is fixed in
  // navigation-contract.zh-CN.md BEFORE any browser result; the projection formula,
  // the default FOV, the yaw/pitch definition and the picking intersection are
  // untouched. The pan is a navigation reference plane, never a terrain height and
  // never a new saved world value: only camera.target.x/z move.
  var PAN_FOV = 0.9;              // the FOV the projection and the picking already use
  var PAN_DRAG_THRESHOLD = 3;     // px, the same threshold the rotate gesture uses
  /// The camera's horizontal right vector and horizontal look direction. Derived
  /// from yaw/pitch only, so a near-horizon view can never run away: no ray is cast
  /// at the ground and no plane is intersected.
  function panBasis(yaw, pitch) {
    var z = [Math.cos(pitch) * Math.sin(yaw), Math.sin(pitch), Math.cos(pitch) * Math.cos(yaw)];
    var right = cross3([0, 1, 0], z);
    var length = Math.hypot(right[0], right[1], right[2]);
    if (length > 1e-6) { right = [right[0] / length, right[1] / length, right[2] / length]; }
    else { right = [Math.cos(yaw), 0, -Math.sin(yaw)]; }
    return { right: right, forward: [-Math.sin(yaw), 0, -Math.cos(yaw)] };
  }
  /// Pure pan function: pointer delta and canvas CSS size in, new target out.
  /// Returns null (camera untouched) for any degenerate or non-finite input.
  function panTargetFromDrag(camera, canvasSize, startTarget, dx, dy) {
    var finite = function (v) { return typeof v === "number" && isFinite(v); };
    if (!camera || !canvasSize || !startTarget) { return null; }
    if (!startTarget.every(finite)) { return null; }
    if (!finite(canvasSize.width) || !finite(canvasSize.height) || canvasSize.width <= 0 || canvasSize.height <= 0) { return null; }
    if (!finite(dx) || !finite(dy)) { return null; }
    if (!finite(camera.distance) || camera.distance <= 0) { return null; }
    var basis = panBasis(camera.yaw, camera.pitch);
    var worldPerPx = 2 * camera.distance * Math.tan(PAN_FOV / 2) / canvasSize.height;
    var target = [
      startTarget[0] - basis.right[0] * dx * worldPerPx + basis.forward[0] * dy * worldPerPx,
      startTarget[1],
      startTarget[2] - basis.right[2] * dx * worldPerPx + basis.forward[2] * dy * worldPerPx
    ];
    if (!target.every(finite)) { return null; }
    return target;
  }
  function canvasCssSize(canvas) {
    var rect = canvas.getBoundingClientRect();
    return { width: rect.width, height: rect.height };
  }
  /// One place that ends a gesture, so pointerup, pointercancel,
  /// lostpointercapture and window blur can never leave a ghost drag behind.
  /// G32-R1: the counters are settled here, because only this exit knows that a whole
  /// gesture has finished: panCount counts completed pan gestures (once, and only when
  /// at least one step was really applied), panMoves counts applied steps one by one.
  function endPointerGesture(reason, event) {
    if (!state.pointerGesture) { return null; }
    var gesture = state.pointerGesture;
    state.pointerGesture = null;
    gesture.endReason = reason;
    gesture.cancelled = reason !== "pointerup";
    gesture.picked = false;
    if (gesture.mode === "pan" && gesture.panApplied === true) { state.panCount++; }
    state.gestureLog.push({ mode: gesture.mode, endReason: reason, cancelled: gesture.cancelled, moved: round6(gesture.moved), picked: false });
    if (state.gestureLog.length > 32) { state.gestureLog.shift(); }
    return gesture;
  }
  /// Applies one pan step from the pointerdown snapshot. Repeated moves apply the
  /// whole delta to the snapshot again, so displacements never accumulate twice and
  /// a symmetric gesture returns to the exact starting target. A rejection is counted
  /// once per gesture (gesture.rejected guards it) and never touches the camera.
  function applyPanStep(gesture, canvas, dx, dy) {
    var target = panTargetFromDrag(state.camera, gesture.canvas, gesture.target, dx, dy);
    if (!target) {
      if (gesture.rejected !== true) { gesture.rejected = true; state.panRejections++; }
      state.lastPanRejection = { dx: round6(dx), dy: round6(dy), reason: "non-finite or degenerate pan input" };
      return false;
    }
    state.camera.target = target;
    state.camera.mode = "free";
    state.cameraMoves++;
    state.panMoves++;
    gesture.panApplied = true;
    state.lastPan = {
      dx: round6(dx), dy: round6(dy),
      worldPerPx: round6(2 * state.camera.distance * Math.tan(PAN_FOV / 2) / gesture.canvas.height),
      from: [round6(gesture.target[0]), round6(gesture.target[1]), round6(gesture.target[2])],
      to: [round6(target[0]), round6(target[1]), round6(target[2])]
    };
    render();
    return true;
  }
  // ------------------------------------------------------------------ G33 minimap
  // A read-only navigation overview. It reuses the SAME projection the renderer and
  // the picking use (0.9 FOV, lookAt, NDC unprojection), so the overview can never
  // disagree with the map, and it keeps its OWN drag state instead of
  // state.pointerGesture so the G32 pointer contract cannot change. Only
  // camera.target.x/z ever move; target.y, yaw, pitch and distance are untouched.
  var MINIMAP_SIZE = 140;         // SVG user units == CSS px of the fixed box
  var MINIMAP_THRESHOLD = 3;      // px, the same drag threshold the map gestures use
  /// A saved window rectangle is usable only when every field is finite and the span is
  /// strictly positive. Nothing here substitutes 1 for an invalid span: an invalid window
  /// makes the mapping unavailable instead of silently drawing a wrong one.
  function minimapWindowValid(win) {
    if (!win) { return false; }
    var nums = [win.minX, win.minZ, win.widthM, win.depthM];
    for (var i = 0; i < 4; i++) { if (typeof nums[i] !== "number" || !isFinite(nums[i])) { return false; } }
    return win.widthM > 0 && win.depthM > 0;
  }
  /// Maps a saved-window world XZ point to minimap SVG user units (0..size).
  /// NORTH IS UP: +Z is south, so a larger z must give a SMALLER svg y.
  /// Pure: no DOM, no camera, and no truncation, so a point outside the window still
  /// projects (the caller decides whether to clamp it).
  function minimapWorldToPx(win, size, x, z) {
    if (!minimapWindowValid(win)) { return null; }
    if (typeof size !== "number" || !isFinite(size) || size <= 0) { return null; }
    if (typeof x !== "number" || !isFinite(x) || typeof z !== "number" || !isFinite(z)) { return null; }
    var px = ((x - win.minX) / win.widthM) * size;
    var py = ((z - win.minZ) / win.depthM) * size;
    if (!isFinite(px) || !isFinite(py)) { return null; }
    return { px: px, py: py };
  }
  /// The exact inverse of minimapWorldToPx (same north-up convention).
  function minimapPxToWorld(win, size, px, py) {
    if (!minimapWindowValid(win)) { return null; }
    if (typeof size !== "number" || !isFinite(size) || size <= 0) { return null; }
    if (typeof px !== "number" || !isFinite(px) || typeof py !== "number" || !isFinite(py)) { return null; }
    var x = win.minX + (px / size) * win.widthM;
    var z = win.minZ + (py / size) * win.depthM;
    if (!isFinite(x) || !isFinite(z)) { return null; }
    return { x: x, z: z };
  }
  /// 2D Sutherland-Hodgman: clip an ordered (x,z) polygon by the half-space f >= 0.
  function minimapClipHalfSpace(poly, f) {
    if (!poly || poly.length < 3) { return []; }
    var out = [];
    for (var i = 0; i < poly.length; i++) {
      var a = poly[i], b = poly[(i + 1) % poly.length];
      var fa = f(a), fb = f(b);
      if (fa >= 0) { out.push(a); }
      if ((fa >= 0) !== (fb >= 0)) {
        var t = fa / (fa - fb);
        out.push([a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])]);
      }
    }
    return out;
  }
  /// The camera's HORIZONTAL REFERENCE-PLANE footprint: the plane y = planeY cut by the six
  /// frustum half-spaces (the four side planes through the eye, the near plane and the far
  /// plane), then intersected with the saved window. It is an ordered convex polygon in world
  /// (x,z). No per-corner clamping, no 60x virtual point, and no "finite means fine":
  /// an empty intersection is reported as empty and invalid input as unavailable.
  /// This is the visible extent of a flat reference plane, NOT what terrain occlusion hides.
  function minimapFootprint(camera, canvasSize, win, planeY) {
    if (!camera || !canvasSize || !minimapWindowValid(win)) { return null; }
    var camNums = [camera.yaw, camera.pitch, camera.distance, planeY, camera.target[0], camera.target[1], camera.target[2]];
    for (var ci = 0; ci < camNums.length; ci++) { if (typeof camNums[ci] !== "number" || !isFinite(camNums[ci])) { return null; } }
    if (typeof canvasSize.width !== "number" || typeof canvasSize.height !== "number" || !(canvasSize.width > 0) || !(canvasSize.height > 0)) { return null; }
    if (!(camera.distance > 0)) { return null; }
    var eye = [
      camera.target[0] + camera.distance * Math.cos(camera.pitch) * Math.sin(camera.yaw),
      camera.target[1] + camera.distance * Math.sin(camera.pitch),
      camera.target[2] + camera.distance * Math.cos(camera.pitch) * Math.cos(camera.yaw)
    ];
    if (!(eye[1] > planeY)) {
      // the reference plane is at or behind the eye level: the exact intersection is at most a
      // line, so nothing may be drawn and no full-window shape may be fabricated
      return { status: "empty", points: [], clippedToWindow: false, reason: "the reference plane is not in front of the camera" };
    }
    var mvp = mat4Multiply(perspective(0.9, canvasSize.width / canvasSize.height, 0.5, 2000), lookAt(eye, camera.target, [0, 1, 0]));
    var inv = invert(mvp);
    if (!inv) { return { status: "unavailable", points: [], clippedToWindow: false, reason: "the view matrix is not invertible" }; }
    function unproject(ndcX, ndcY, z) {
      var v = [ndcX, ndcY, z, 1], o = [0, 0, 0, 0];
      for (var k = 0; k < 4; k++) { o[k] = inv[0 * 4 + k] * v[0] + inv[1 * 4 + k] * v[1] + inv[2 * 4 + k] * v[2] + inv[3 * 4 + k] * v[3]; }
      return [o[0] / o[3], o[1] / o[3], o[2] / o[3]];
    }
    // the four corner rays, normalized
    var corners = [[1, -1], [1, 1], [-1, 1], [-1, -1]];
    var rays = [];
    for (var i = 0; i < 4; i++) {
      var near = unproject(corners[i][0], corners[i][1], -1), far = unproject(corners[i][0], corners[i][1], 1);
      var dx = far[0] - near[0], dy = far[1] - near[1], dz = far[2] - near[2];
      var len = Math.hypot(dx, dy, dz);
      if (!(len > 0) || !isFinite(len)) { return { status: "unavailable", points: [], clippedToWindow: false, reason: "a corner ray is degenerate" }; }
      rays.push([dx / len, dy / len, dz / len]);
    }
    var forward = [-Math.cos(camera.pitch) * Math.sin(camera.yaw), -Math.sin(camera.pitch), -Math.cos(camera.pitch) * Math.cos(camera.yaw)];
    function inward(a, b) {
      var n = cross3(a, b);
      var l = Math.hypot(n[0], n[1], n[2]);
      if (!(l > 1e-12)) { return null; }
      n = [n[0] / l, n[1] / l, n[2] / l];
      if (dot3(n, forward) < 0) { n = [-n[0], -n[1], -n[2]]; }
      return n;
    }
    var planeNormals = [];
    for (var p = 0; p < 4; p++) {
      var n = inward(rays[p], rays[(p + 1) % 4]);
      if (!n) { return { status: "unavailable", points: [], clippedToWindow: false, reason: "two corner rays are parallel" }; }
      planeNormals.push(n);
    }
    // start from a square that comfortably contains the window and the near ground region; the
    // side planes and the far plane bound it, so this square is never the visible limit
    var span = Math.max(win.widthM, win.depthM, camera.distance) * 20;
    var poly = [[eye[0] - span, eye[2] - span], [eye[0] + span, eye[2] - span], [eye[0] + span, eye[2] + span], [eye[0] - span, eye[2] + span]];
    for (var q = 0; q < 4; q++) {
      var nq = planeNormals[q];
      poly = minimapClipHalfSpace(poly, function (pt) { return nq[0] * (pt[0] - eye[0]) + nq[2] * (pt[1] - eye[2]) + nq[1] * (planeY - eye[1]); });
      if (poly.length < 3) { return { status: "empty", points: [], clippedToWindow: false, reason: "the ground plane is outside the view frustum" }; }
    }
    poly = minimapClipHalfSpace(poly, function (pt) { return dot3(forward, [pt[0] - eye[0], planeY - eye[1], pt[1] - eye[2]]) - 0.5; });
    if (poly.length < 3) { return { status: "empty", points: [], clippedToWindow: false, reason: "only the near plane reaches the ground" }; }
    poly = minimapClipHalfSpace(poly, function (pt) { return 2000 - dot3(forward, [pt[0] - eye[0], planeY - eye[1], pt[1] - eye[2]]); });
    if (poly.length < 3) { return { status: "empty", points: [], clippedToWindow: false, reason: "the ground intersection is beyond the far plane" }; }
    var windowPoly = poly;
    // G33-R2: the clip is tracked per side and per change, so "clipped" can never be a constant.
    // A side clip counts as a change when it inserts an intersection point, drops a vertex, or
    // rewrites a vertex coordinate. Comparing only the array length would miss the case where the
    // vertex count happens to stay the same while the coordinates were cut.
    var clipChanged = false;
    var windowSides = [
      function (pt) { return pt[0] - win.minX; },
      function (pt) { return win.minX + win.widthM - pt[0]; },
      function (pt) { return pt[1] - win.minZ; },
      function (pt) { return win.minZ + win.depthM - pt[1]; }
    ];
    for (var ws = 0; ws < windowSides.length; ws++) {
      var next = minimapClipHalfSpace(windowPoly, windowSides[ws]);
      if (next.length !== windowPoly.length) { clipChanged = true; }
      for (var vi = 0; vi < next.length && !clipChanged; vi++) {
        var match = false;
        for (var vj = 0; vj < windowPoly.length; vj++) {
          if (next[vi][0] === windowPoly[vj][0] && next[vi][1] === windowPoly[vj][1]) { match = true; break; }
        }
        if (!match) { clipChanged = true; }
      }
      windowPoly = next;
    }
    if (windowPoly.length < 3) { return { status: "empty", points: [], clippedToWindow: false, reason: "the visible ground region does not meet the saved window" }; }
    // cross-check with a geometric predicate: are ALL frustum polygon vertices inside the window?
    // The two must agree; the predicate is what makes the flag meaningful rather than incidental.
    // G33-R2: the tolerance is scale-relative and comfortably ABOVE this code's own float noise.
    // Unprojecting through a 4x4 inverse and a closed-form derivation differ by up to ~1e-4 m at
    // these coordinates (measured 2e-6 .. 1e-4 m), so a 6.4e-8 window would let a vertex sitting
    // on the boundary flip the flag between runs. 1e-6 * magnitude = 3.2e-4 m here, which is
    // still far below the millimetre facts the window holds.
    var coordScale = Math.max(1, Math.abs(win.minX), Math.abs(win.minZ), Math.abs(win.minX + win.widthM), Math.abs(win.minZ + win.depthM), win.widthM, win.depthM);
    var eps = 1e-6 * coordScale;
    var allInside = true;
    for (var fi = 0; fi < poly.length; fi++) {
      var fp = poly[fi];
      if (fp[0] < win.minX - eps || fp[0] > win.minX + win.widthM + eps || fp[1] < win.minZ - eps || fp[1] > win.minZ + win.depthM + eps) { allInside = false; break; }
    }
    if (allInside) { clipChanged = false; }
    return { status: "ok", points: windowPoly, frustumPoints: poly, clippedToWindow: clipChanged === true, frustumInsideWindow: allInside === true, insideEpsilonM: eps };
  }
  // ---------------------------------------------------------------------------------------------
  // G34: terrain-occlusion visibility of the mapped ground.
  //
  // The minimap already draws the horizontal reference-plane footprint. That footprint says which
  // GROUND AREA the view covers; it says nothing about whether the saved terrain hides part of it.
  // This block answers exactly that question on a fixed 16x16 sampling grid:
  //
  //   visible   - the sampled ground point can be reached from the camera eye
  //   hidden    - a saved terrain triangle stands between the eye and that point
  //   undefined - the cell cannot be judged (outside the footprint, bad surface, degenerate line)
  //
  // It is a SAMPLING GRID over the saved terrain triangles, NOT per-pixel or photon-accurate
  // visibility. It never invents data: it only reads the already-loaded surface.
  // ---------------------------------------------------------------------------------------------
  var MINIMAP_VISIBILITY_RESOLUTION = 16;
  var MINIMAP_VISIBILITY_LIFT_M = 0.02;
  var MINIMAP_VISIBILITY_EPS_M = 0.001;
  // G34-R1: the frustum limits must match the ones the renderer uses, so "outside-view" means the
  // same thing here as it does on screen.
  var MINIMAP_VISIBILITY_FOV = 0.9;
  var MINIMAP_VISIBILITY_NEAR_M = 0.5;
  var MINIMAP_VISIBILITY_FAR_M = 2000;
  // G34-R2: spatial bucket size for the occlusion index. Bigger buckets mean a sight line touches
  // fewer of them; the per-cell de-duplication then removes the overlaps. This is a pure PERFORMANCE
  // knob: the candidate set is always a superset of the triangles near the sight line, so the
  // classification is unchanged (asserted by a before/after per-cell diff).
  var MINIMAP_VISIBILITY_BUCKET_M = 4;
  /// Bucket grid size for the occlusion index. See MINIMAP_VISIBILITY_BUCKET_M.
  var occlusionIndexCache = null;
  /// G34-R3: the de-duplication stamp lifecycle. The previous revision used a counter that RESTARTED
  /// at 1 for every grid while the stamp array lived on inside the cached index, so the second frame
  /// found stamps equal to its own fresh markers and skipped triangles it had never tested -- the same
  /// wall read hidden on frame 1 and visible on frame 2. The epoch is now monotonic across calls and
  /// shared by every user of the array, and it is reset explicitly (clearing the stamps) before it
  /// could overflow.
  var occlusionEpoch = 0;
  var OCCLUSION_EPOCH_LIMIT = 2140000000;
  /// G34-R4: every rendered visibility snapshot gets a monotonic id, so a reader can tell WHICH render
  /// produced a given set of numbers instead of mixing readings from different calls.
  var visibilitySnapshotSerial = 0;
  /// A stable, order-sensitive digest of the footprint polygon, so "same domain" is checkable rather
  /// than assumed.
  function visibilityFootprintHash(fp) {
    if (!fp || fp.status !== "ok" || !fp.points || fp.points.length < 3) { return fp ? String(fp.status) : "none"; }
    var h = 2166136261;
    for (var i = 0; i < fp.points.length; i++) {
      var a = String(round6(fp.points[i][0])) + "," + String(round6(fp.points[i][1])) + ";";
      for (var k = 0; k < a.length; k++) { h ^= a.charCodeAt(k); h = (h * 16777619) >>> 0; }
    }
    return "fnv1a:" + h.toString(16) + ":" + fp.points.length;
  }
  /// Reserve the next unique marker. Never returns a value that has been handed out before.
  function nextOcclusionEpoch(clearStamps) {
    if (occlusionEpoch >= OCCLUSION_EPOCH_LIMIT) {
      // an explicit, observable reset: clear every stamp so old markers cannot be confused with new
      if (clearStamps) { clearStamps.fill(0); }
      occlusionEpoch = 0;
    }
    occlusionEpoch++;
    return occlusionEpoch;
  }
  /// G34-R2 conservative pre-filter. For every triangle we keep the rectangle it covers in XZ and
  /// its HIGHEST vertex. The grid then stores, per (ix, iz) cell of the terrain lattice, the largest
  /// of those heights over every triangle whose XZ rectangle touches the cell. Because a triangle's
  /// interpolated height can never exceed the largest of its own vertex heights, this per-cell value
  /// is an UPPER BOUND of the terrain height anywhere inside that cell. That is the whole point: any
  /// sight-line sample that stays above the bound cannot be occluded by that cell, and the exact
  /// triangle test can be skipped for it without ever missing an occlusion.
  function occlusionBuildHeightBounds(index) {
    if (index.heightBounds) { return index.heightBounds; }
    var w = index.grid.sampleWidth, d = index.grid.sampleDepth;
    var bounds = new Float64Array((w - 1) * (d - 1));
    for (var i = 0; i < bounds.length; i++) { bounds[i] = -Infinity; }
    var cellTris = index.buckets;
    var cell = index.cell;
    for (var gz = 0; gz < index.gd; gz++) {
      for (var gx = 0; gx < index.gw; gx++) {
        var bucket = cellTris[gz * index.gw + gx];
        if (!bucket) { continue; }
        for (var b = 0; b < bucket.length; b++) {
          var tr = bucket[b].tri;
          var highest = Math.max(tr[1], tr[4], tr[7]);
          var minTX = Math.min(tr[0], tr[3], tr[6]), maxTX = Math.max(tr[0], tr[3], tr[6]);
          var minTZ = Math.min(tr[2], tr[5], tr[8]), maxTZ = Math.max(tr[2], tr[5], tr[8]);
          var ix0 = Math.max(0, Math.ceil((minTX - index.baseX) / index.stepX - 1));
          var ix1 = Math.min(w - 2, Math.floor((maxTX - index.baseX) / index.stepX));
          var iz0 = Math.max(0, Math.ceil((minTZ - index.baseZ) / index.stepZ - 1));
          var iz1 = Math.min(d - 2, Math.floor((maxTZ - index.baseZ) / index.stepZ));
          for (var iz = iz0; iz <= iz1; iz++) {
            for (var ix = ix0; ix <= ix1; ix++) {
              var k = iz * (w - 1) + ix;
              if (highest > bounds[k]) { bounds[k] = highest; }
            }
          }
        }
      }
    }
    void cell;
    index.heightBounds = bounds;
    return bounds;
  }
  /// Could the sight line reach the terrain inside the cell that contains fraction (fx, fz)? Returns
  /// false only when the line is provably ABOVE the upper bound for the whole crossing of that cell,
  /// which makes an exact triangle test unnecessary. A true answer is always safe.
  /// Upper bounds for the sight line: is the line provably ABOVE the terrain for its whole length?
  /// The line is sampled at a fine, conservative step; at each sample the enclosing terrain cell's
  /// upper bound is read and the NEXT cell along the line is also checked, so a crossing between two
  /// samples cannot slip through. A `false` answer is always safe (the exact test then decides); a
  /// `true` answer means no triangle can stand on the line, so the exact test is skipped.
  function occlusionLineClearsTerrain(index, bounds, eye, dir, len) {
    var w = index.grid.sampleWidth, d = index.grid.sampleDepth;
    var stepM = Math.min(index.stepX, index.stepZ) * 0.5;
    var steps = Math.ceil(len / stepM);
    if (steps <= 0) { return false; }
    for (var k = 0; k <= steps; k++) {
      var m = Math.min(len, k * stepM);
      var x = eye[0] + dir[0] * m, y = eye[1] + dir[1] * m, z = eye[2] + dir[2] * m;
      var fx = (x - index.baseX) / index.stepX, fz = (z - index.baseZ) / index.stepZ;
      if (fx < 0 || fz < 0 || fx >= w - 1 || fz >= d - 1) { continue; }
      var ix = Math.floor(fx), iz = Math.floor(fz);
      var bound = bounds[iz * (w - 1) + ix];
      // the next cell along the line, so a boundary crossing between samples is still covered
      var nx = ix + (dir[0] >= 0 ? 1 : -1), nz = iz + (dir[2] >= 0 ? 1 : -1);
      var bound2 = (nx >= 0 && nz >= 0 && nx < w - 1 && nz < d - 1) ? bounds[nz * (w - 1) + nx] : -Infinity;
      var worst = Math.max(bound, bound2);
      if (worst === -Infinity) { continue; }
      if (y <= worst + MINIMAP_VISIBILITY_EPS_M) { return false; }
    }
    return true;
  }
  function occlusionCellCanOcclude(index, bounds, eye, dir, len, fx, fz) {
    var w = index.grid.sampleWidth, d = index.grid.sampleDepth;
    var ix = Math.max(0, Math.min(w - 2, Math.floor(fx)));
    var iz = Math.max(0, Math.min(d - 2, Math.floor(fz)));
    var bound = bounds[iz * (w - 1) + ix];
    if (bound === -Infinity) { return false; }
    // the parameter range over which the line crosses this cell, clamped to the segment
    var tx0 = (ix * index.stepX + index.baseX - eye[0]), tx1 = ((ix + 1) * index.stepX + index.baseX - eye[0]);
    var tz0 = (iz * index.stepZ + index.baseZ - eye[2]), tz1 = ((iz + 1) * index.stepZ + index.baseZ - eye[2]);
    var lo = 0, hi = len;
    if (Math.abs(dir[0]) > 1e-12) {
      var a = tx0 / dir[0], b = tx1 / dir[0];
      lo = Math.max(lo, Math.min(a, b)); hi = Math.min(hi, Math.max(a, b));
    } else if (eye[0] < Math.min(tx0, tx1) || eye[0] > Math.max(tx0, tx1)) { return false; }
    if (Math.abs(dir[2]) > 1e-12) {
      var c = tz0 / dir[2], e = tz1 / dir[2];
      lo = Math.max(lo, Math.min(c, e)); hi = Math.min(hi, Math.max(c, e));
    } else if (eye[2] < Math.min(tz0, tz1) || eye[2] > Math.max(tz0, tz1)) { return false; }
    if (hi < lo) { return false; }
    if (hi < 0 || lo > len) { return false; }
    // the lowest the line can be while inside the cell
    var lowest = eye[1] + dir[1] * (dir[1] >= 0 ? lo : hi);
    return lowest <= bound + MINIMAP_VISIBILITY_EPS_M;
  }

  /// Discard cells that leave the saved window. Unlike the footprint clip this PRESERVES null
  /// (an unjudged cell), because dropping them would silently turn "unknown" into "not sampled".
  function minimapClipCells(cells, f) {
    if (!cells || cells.length < 3) { return []; }
    var out = [];
    for (var i = 0; i < cells.length; i++) {
      var a = cells[i], b = cells[(i + 1) % cells.length];
      var fa = a === null ? null : f(a), fb = b === null ? null : f(b);
      var ia = fa === null ? false : fa >= 0, ib = fb === null ? false : fb >= 0;
      if (ia) { out.push(a); } else if (a === null) { out.push(null); }
      if (ia !== ib) {
        if (fa === null || fb === null) {
          // an unjudged cell may not create an intersection point; carry the known side through
          if (ia) { out.push(a); } else if (ib) { out.push(b); }
        } else {
          var t = fa / (fa - fb);
          out.push([a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])]);
        }
      }
    }
    return out;
  }
  function minimapCellsInside(cells) {
    return cells.filter(function (c) { return c !== null; });
  }
  function minimapPointInPolygon(poly, x, z) {
    var inside = false;
    for (var i = 0, j = poly.length - 1; i < poly.length; j = i++) {
      var xi = poly[i][0], zi = poly[i][1], xj = poly[j][0], zj = poly[j][1];
      if (((zi > z) !== (zj > z)) && (x < ((xj - xi) * (z - zi)) / (zj - zi) + xi)) { inside = !inside; }
    }
    return inside;
  }
  /// Lazily build an XZ bucket index over one surface's terrain triangles. This is a read-only
  /// acceleration structure: it copies no geometry and changes nothing about the model.
  function occlusionIndexFor(name) {
    if (occlusionIndexCache && occlusionIndexCache.name === name) { return occlusionIndexCache; }
    var surface = model && model.surfaces ? model.surfaces[name] : null;
    var grid = model && model.grid ? model.grid[name] : null;
    if (!surface || !grid || !surface.verticesMetres || !grid.sampleWidth || !grid.sampleDepth) { return null; }
    var verts = surface.verticesMetres;
    var indices;
    try { indices = terrainIndicesFor(name); } catch (error) { return null; }
    if (!indices || indices.length < 3) { return null; }
    var w = grid.sampleWidth, d = grid.sampleDepth;
    // G34 fix: the terrain's own extent, NOT the saved window's. They are not the same (the terrain
    // spans 63 m of a 64 m window), and using the window span put every boundary cell half a metre
    // off and made cells at the far edge "undefined".
    var v0x = verts[0], v0z = verts[2];
    var vlast = (w * d - 1) * 3;
    var v1x = verts[vlast], v1z = verts[vlast + 2];
    var stepX = (v1x - v0x) / (w - 1), stepZ = (v1z - v0z) / (d - 1);
    if (!(stepX > 0) || !(stepZ > 0)) { return null; }
    // Buckets cover the terrain extent plus a 2 m margin, so the camera eye (which sits outside the
    // window) and every sampled cell always land inside a bucket.
    var margin = 2 * Math.max(stepX, stepZ);
    // G34-R2: with 1 m buckets a long sight line touched about two thousand buckets and pulled in
    // most of the terrain, so de-duplication saved little. Coarser buckets (default 4 m) mean the
    // same line touches roughly a hundred buckets, and the de-dupe then removes the remainder.
    var cell = MINIMAP_VISIBILITY_BUCKET_M;
    var minX = Math.min(v0x, v1x) - margin, minZ = Math.min(v0z, v1z) - margin;
    var gw = Math.max(1, Math.ceil((Math.abs(v1x - v0x) + 2 * margin) / cell));
    var gd = Math.max(1, Math.ceil((Math.abs(v1z - v0z) + 2 * margin) / cell));
    var buckets = new Array(gw * gd);
    var triangleCount = 0;
    for (var i = 0; i + 2 < indices.length; i += 3) {
      var ia = indices[i], ib = indices[i + 1], ic = indices[i + 2];
      if (ia * 3 + 2 >= verts.length || ib * 3 + 2 >= verts.length || ic * 3 + 2 >= verts.length) { continue; }
      var ax = verts[ia * 3], ay = verts[ia * 3 + 1], az = verts[ia * 3 + 2];
      var bx = verts[ib * 3], by = verts[ib * 3 + 1], bz = verts[ib * 3 + 2];
      var cx = verts[ic * 3], cy = verts[ic * 3 + 1], cz = verts[ic * 3 + 2];
      if (![ax, ay, az, bx, by, bz, cx, cy, cz].every(function (v) { return typeof v === "number" && isFinite(v); })) { continue; }
      var lo = [Math.min(ax, bx, cx), Math.min(az, bz, cz)];
      var hi = [Math.max(ax, bx, cx), Math.max(az, bz, cz)];
      var gx0 = Math.max(0, Math.min(gw - 1, Math.floor((lo[0] - minX) / cell)));
      var gx1 = Math.max(0, Math.min(gw - 1, Math.floor((hi[0] - minX) / cell)));
      var gz0 = Math.max(0, Math.min(gd - 1, Math.floor((lo[1] - minZ) / cell)));
      var gz1 = Math.max(0, Math.min(gd - 1, Math.floor((hi[1] - minZ) / cell)));
      triangleCount++;
      for (var gz = gz0; gz <= gz1; gz++) {
        for (var gx = gx0; gx <= gx1; gx++) {
          var key = gz * gw + gx;
          if (!buckets[key]) { buckets[key] = []; }
          // G34-R2: each candidate carries a STABLE key (its position in the flat triangle list) plus
          // its 9 numbers, so the per-cell de-duplication can use a stamp array instead of allocating
          // and comparing arrays. The key is never shown to the classifier.
          buckets[key].push({ key: triangleCount - 1, tri: [ax, ay, az, bx, by, bz, cx, cy, cz] });
        }
      }
    }
    // scratch space for the de-duplication: `dedupeStamp[key]` holds the marker of the cell that last
    // tested this triangle, and `candidates` is reused as the de-duplicated list
    occlusionIndexCache = {
      name: name, cell: cell, gw: gw, gd: gd, buckets: buckets, minX: minX, minZ: minZ,
      triangleCount: triangleCount, grid: { sampleWidth: w, sampleDepth: d },
      baseX: v0x, baseZ: v0z, stepX: stepX, stepZ: stepZ,
      dedupeStamp: new Int32Array(triangleCount + 1), candidates: new Array(triangleCount + 1)
    };
    return occlusionIndexCache;
  }
  /// Candidates near a sight line, gathered by walking the buckets the line actually CROSSES (a 2D
  /// DDA) instead of by its whole bounding box. G34-R2: the bounding box of a long oblique line
  /// covered most of the terrain, which is why the previous revision pulled in thousands of
  /// triangles per cell. A DDA visits only the crossed buckets, so the candidate set stays small
  /// while still being a superset of the triangles the line can hit (each bucket is dilated by one
  /// bucket in every direction to cover triangles that merely overlap the line's neighbourhood).
  function occlusionCandidates(index, x0, z0, x1, z1) {
    var out = [];
    var cell = index.cell;
    var gx0 = Math.floor((x0 - index.minX) / cell), gz0 = Math.floor((z0 - index.minZ) / cell);
    var gx1 = Math.floor((x1 - index.minX) / cell), gz1 = Math.floor((z1 - index.minZ) / cell);
    var dx = gx1 - gx0, dz = gz1 - gz0;
    var n = Math.max(Math.abs(dx), Math.abs(dz));
    if (n === 0) {
      // a very short sight line: just the neighbourhood of its own bucket
      for (var oz = -1; oz <= 1; oz++) {
        for (var ox = -1; ox <= 1; ox++) {
          appendBucket(out, index, gx0 + ox, gz0 + oz);
        }
      }
      return out;
    }
    var stepX = dx >= 0 ? 1 : -1, stepZ = dz >= 0 ? 1 : -1;
    void stepX; void stepZ;
    // Walk the crossed buckets, dilated by ONE bucket. The dilation covers a triangle that straddles
    // a bucket boundary; the exact test then rejects everything else. (A larger dilation tripled the
    // candidate count without changing a single classification.)
    for (var t = 0; t <= n; t++) {
      var fi = t / n;
      var gx = Math.round(gx0 + dx * fi), gz = Math.round(gz0 + dz * fi);
      for (var oz2 = -1; oz2 <= 1; oz2++) {
        for (var ox2 = -1; ox2 <= 1; ox2++) {
          appendBucket(out, index, gx + ox2, gz + oz2);
        }
      }
    }
    return out;
  }
  /// Append one bucket's candidates, skipping out-of-range buckets. Bucket contents are shared
  /// objects, so this pushes references, never copies.
  function appendBucket(out, index, gx, gz) {
    if (gx < 0 || gz < 0 || gx >= index.gw || gz >= index.gd) { return; }
    var bucket = index.buckets[gz * index.gw + gx];
    if (!bucket) { return; }
    for (var i = 0; i < bucket.length; i++) { out.push(bucket[i]); }
  }
  /// Terrain height at (x, z) by interpolating the SAVED triangles, using the same quad split the
  /// draw and pick paths use. Returns null when the point is outside the terrain or the quad is
  /// degenerate, so the caller can report "undefined" instead of guessing.
  function occlusionTerrainHeight(index, x, z) {
    if (!index) { return null; }
    if (!isFinite(x) || !isFinite(z)) { return null; }
    var w = index.grid.sampleWidth, d = index.grid.sampleDepth;
    var verts = (model.surfaces[index.name] || {}).verticesMetres;
    if (!verts) { return null; }
    // the terrain's own spacing, with a relative margin so a cell whose centre lands exactly on the
    // outer edge is interpolated instead of being reported as unknown
    var fx = (x - index.baseX) / index.stepX, fz = (z - index.baseZ) / index.stepZ;
    var margin = 1e-6;
    if (fx < -margin || fz < -margin || fx > w - 1 + margin || fz > d - 1 + margin) { return null; }
    var ix = Math.max(0, Math.min(w - 2, Math.floor(fx)));
    var iz = Math.max(0, Math.min(d - 2, Math.floor(fz)));
    function at(gx, gz) {
      var k = (gz * w + gx) * 3;
      if (k + 2 >= verts.length) { return null; }
      return [verts[k], verts[k + 1], verts[k + 2]];
    }
    var a = at(ix, iz), b = at(ix + 1, iz), c = at(ix, iz + 1), e = at(ix + 1, iz + 1);
    if (!a || !b || !c || !e) { return null; }
    function bary(p, q, r) {
      var den = (q[2] - r[2]) * (p[0] - r[0]) + (r[0] - q[0]) * (p[2] - r[2]);
      if (Math.abs(den) < 1e-12) { return null; }
      var l1 = ((q[2] - r[2]) * (x - r[0]) + (r[0] - q[0]) * (z - r[2])) / den;
      var l2 = ((r[2] - p[2]) * (x - r[0]) + (p[0] - r[0]) * (z - r[2])) / den;
      return [l1, l2, 1 - l1 - l2];
    }
    var split = [[a, c, b], [b, c, e]];
    for (var s = 0; s < 2; s++) {
      var w3 = bary(split[s][0], split[s][1], split[s][2]);
      if (w3 && w3[0] >= -1e-9 && w3[1] >= -1e-9 && w3[2] >= -1e-9) {
        // G34-R2: a non-finite vertex makes the interpolation non-finite. That is MISSING DATA for
        // this cell, so it must return null (the caller reports unknown-data) rather than a NaN that
        // would silently poison every later comparison.
        var h = w3[0] * split[s][0][1] + w3[1] * split[s][1][1] + w3[2] * split[s][2][1];
        if (typeof h !== "number" || !isFinite(h)) { return null; }
        return h;
      }
    }
    // no split contained the point: the quad is degenerate here, so this cell has no usable height
    return null;
  }
  /// Independent segment/triangle intersection for the occlusion question. It is deliberately a
  /// SEPARATE implementation from the main-map picking routine: the pick path must stay untouched,
  /// and the two agreeing is itself a useful cross-check.
  /// Segment/triangle intersection for the occlusion question, in CONTRACT UNITS:
  /// `dir` is a UNIT vector, the returned value is the distance in METRES, and a hit counts only
  /// when it lies strictly between the origin tolerance and the caller's metre limit.
  /// G34-R1: the previous revision compared a metre-scale limit against a normalised parameter,
  /// which silently reduced the search to roughly the first metre of the sight line. The unit is
  /// now explicit in the names so the two can never be mixed again.
  function occlusionSegmentHitMetres(o, dir, minMetres, maxMetres, tri) {
    // G34-R2: written with SCALARS and no temporary arrays. The previous revision allocated about
    // six arrays per test through sub3/cross3/dot3, which at ~250k tests per grid dominated the
    // cost. The arithmetic is identical, term for term.
    var ax = tri[0], ay = tri[1], az = tri[2];
    var e1x = tri[3] - ax, e1y = tri[4] - ay, e1z = tri[5] - az;
    var e2x = tri[6] - ax, e2y = tri[7] - ay, e2z = tri[8] - az;
    var dx = dir[0], dy = dir[1], dz = dir[2];
    var pvx = dy * e2z - dz * e2y, pvy = dz * e2x - dx * e2z, pvz = dx * e2y - dy * e2x;
    var det = e1x * pvx + e1y * pvy + e1z * pvz;
    if (det < 1e-12 && det > -1e-12) { return null; }
    var inv = 1 / det;
    var tvx = o[0] - ax, tvy = o[1] - ay, tvz = o[2] - az;
    var u = (tvx * pvx + tvy * pvy + tvz * pvz) * inv;
    if (u < -1e-9 || u > 1 + 1e-9) { return null; }
    var qvx = tvy * e1z - tvz * e1y, qvy = tvz * e1x - tvx * e1z, qvz = tvx * e1y - tvy * e1x;
    var v = (dx * qvx + dy * qvy + dz * qvz) * inv;
    if (v < -1e-9 || u + v > 1 + 1e-9) { return null; }
    // because `dir` is a unit vector this parameter IS the distance in metres, and the contract
    // window is minMetres < s < maxMetres (both metres, both derived from the same epsilon)
    var s = (e2x * qvx + e2y * qvy + e2z * qvz) * inv;
    if (!(s > minMetres) || !(s < maxMetres)) { return null; }
    return s;
  }
  /// The signed distance to the plane of a triangle along the sight line, or null when the line is
  /// parallel. Used only for the explicit reason strings, never for the classification itself.
  function occlusionPlaneDistanceMetres(o, dir, tri) {
    var normal = cross3(sub3([tri[3], tri[4], tri[5]], [tri[0], tri[1], tri[2]]), sub3([tri[6], tri[7], tri[8]], [tri[0], tri[1], tri[2]]));
    var den = dot3(normal, dir);
    if (Math.abs(den) < 1e-12) { return null; }
    return dot3(normal, sub3([tri[0], tri[1], tri[2]], o)) / den;
  }
  /// Classify one surface sample against the saved terrain.
  /// Returns `{ klass, reason, sMetres, lengthMetres, epsilonM, candidateCount, testedCount, dedupedCount }`
  /// so every decision carries the numbers it was made from, including how much work it cost.
  /// G34-R2: the candidate list is DE-DUPLICATED with a stamp array before any intersection test, so
  /// a triangle reached through several buckets is tested once; buckets are still clipped to the
  /// sight line's own bounding box, so a long sight line does not walk the whole terrain.
  function occlusionClassifyDetailed(index, eye, x, z, referenceY, perf, marker, dedupeStamp, dedupeList) {
    var out = { klass: "undefined", reason: "unknown-data", sMetres: null, lengthMetres: null, epsilonM: MINIMAP_VISIBILITY_EPS_M, candidateCount: 0, testedCount: 0, dedupedCount: 0 };
    if (!index || !eye) { out.reason = "unknown-data: no terrain index or no observer position"; return out; }
    if (![eye[0], eye[1], eye[2], x, z, referenceY].every(function (v) { return typeof v === "number" && isFinite(v); })) { out.reason = "unknown-data: a non-finite coordinate"; return out; }
    var target = [x, referenceY + MINIMAP_VISIBILITY_LIFT_M, z];
    var d = sub3(target, eye);
    var len = Math.hypot(d[0], d[1], d[2]);
    out.lengthMetres = len;
    if (!(len > MINIMAP_VISIBILITY_EPS_M)) { out.reason = "numerical-degenerate: the sight line is shorter than the epsilon"; return out; }
    var dir = [d[0] / len, d[1] / len, d[2] / len];
    var maxMetres = len - MINIMAP_VISIBILITY_EPS_M;
    // G34-R2 fast path: when the whole sight line is provably above the terrain's per-cell upper
    // bounds, no exact triangle test can find an occluder, so the cell is visible by construction.
    var bounds = occlusionBuildHeightBounds(index);
    if (occlusionLineClearsTerrain(index, bounds, eye, dir, len)) {
      out.klass = "visible";
      out.reason = "the sight line stays above the terrain upper-bound field for its whole length";
      out.prefilterCleared = true;
      if (perf) { perf.prefilterCleared++; }
      return out;
    }
    var candidates = occlusionCandidates(index, eye[0], eye[2], x, z);
    out.candidateCount = candidates ? candidates.length : 0;
    if (perf) { perf.candidateTriangles += out.candidateCount; }
    if (!candidates || !candidates.length) { out.reason = "unknown-data: the saved terrain has no triangle near this sight line"; return out; }
    // de-duplicate: a triangle in several buckets must be tested once. The caller owns the stamp
    // array and the reusable list, so this allocates nothing per cell.
    var stamp = dedupeStamp || index.dedupeStamp;
    var list = dedupeList || index.candidates;
    var kept = 0;
    for (var ci = 0; ci < candidates.length; ci++) {
      var cand = candidates[ci];
      var key = cand.key;
      if (stamp[key] === marker) { continue; }
      stamp[key] = marker;
      list[kept] = cand.tri;
      kept++;
    }
    out.testedCount = kept;
    out.dedupedCount = out.candidateCount - kept;
    if (perf) { perf.intersectionTests += kept; perf.dedupedTriangles += out.dedupedCount; }
    var best = null;
    for (var ti = 0; ti < kept; ti++) {
      var s = occlusionSegmentHitMetres(eye, dir, MINIMAP_VISIBILITY_EPS_M, maxMetres, list[ti]);
      if (s === null) { continue; }
      if (best === null || s < best) { best = s; }
    }
    if (best !== null) { out.klass = "hidden"; out.reason = "a saved terrain triangle stands " + round6(best) + " m from the observer along a " + round6(len) + " m sight line"; out.sMetres = best; return out; }
    out.klass = "visible";
    out.reason = "no saved terrain triangle crosses the sight line before the target";
    return out;
  }
  /// Backwards-compatible wrapper for the published test seam: returns just the class name.
  function occlusionClassify(index, eye, x, z, referenceY) {
    return occlusionClassifyDetailed(index, eye, x, z, referenceY).klass;
  }
  /// The visibility grid: G33 footprint ∩ saved window, sampled on a fixed grid. Every expectation
  /// about this function can be recomputed by an independent oracle; it never reads its own output.
  /// G34-R1: is this surface sample inside the camera's near/far frustum? A surface point inside the
  /// G33 footprint but OUTSIDE the view has not been judged visible -- it is `outside-view`, which
  /// is a different thing from `unknown-data` and from `hidden`.
  function occlusionInsideFrustum(camera, canvasSize, world, sampleY) {
    if (!camera || !canvasSize || !(canvasSize.height > 0)) { return true; }
    var eye = [
      camera.target[0] + camera.distance * Math.cos(camera.pitch) * Math.sin(camera.yaw),
      camera.target[1] + camera.distance * Math.sin(camera.pitch),
      camera.target[2] + camera.distance * Math.cos(camera.pitch) * Math.cos(camera.yaw)
    ];
    var f = sub3(camera.target, eye);
    var fl = Math.hypot(f[0], f[1], f[2]);
    if (!(fl > 0)) { return true; }
    f = [f[0] / fl, f[1] / fl, f[2] / fl];
    var r = cross3([0, 1, 0], [-f[0], -f[1], -f[2]]);
    var rl = Math.hypot(r[0], r[1], r[2]);
    if (!(rl > 1e-9)) { r = [Math.cos(camera.yaw), 0, -Math.sin(camera.yaw)]; } else { r = [r[0] / rl, r[1] / rl, r[2] / rl]; }
    var u = cross3([-f[0], -f[1], -f[2]], r);
    var ul = Math.hypot(u[0], u[1], u[2]);
    u = [u[0] / ul, u[1] / ul, u[2] / ul];
    var rel = sub3([world.x, sampleY, world.z], eye);
    var along = dot3(rel, f);
    if (along < MINIMAP_VISIBILITY_NEAR_M || along > MINIMAP_VISIBILITY_FAR_M) { return false; }
    var tanV = Math.tan(MINIMAP_VISIBILITY_FOV / 2), tanH = tanV * (canvasSize.width / canvasSize.height);
    // outside a side plane when |lateral| exceeds the half-extent at this depth
    if (Math.abs(dot3(rel, r)) > along * tanH + MINIMAP_VISIBILITY_EPS_M) { return false; }
    if (Math.abs(dot3(rel, u)) > along * tanV + MINIMAP_VISIBILITY_EPS_M) { return false; }
    return true;
  }
  /// Is the observer itself buried in, or touching, the saved terrain? Such a viewpoint is a
  /// degenerate observer: its occlusion result is not evidence about the terrain, so the whole grid
  /// is reported unavailable with an explicit reason instead of a mountain "positive" example.
  function occlusionObserverIsDegenerate(index, eye) {
    if (!index || !eye) { return false; }
    var h = occlusionTerrainHeight(index, eye[0], eye[2]);
    if (h === null) { return false; }
    return eye[1] <= h + MINIMAP_VISIBILITY_EPS_M;
  }
  function minimapVisibilityGrid(camera, canvasSize, win, footprint) {
    var resolution = { width: MINIMAP_VISIBILITY_RESOLUTION, height: MINIMAP_VISIBILITY_RESOLUTION };
    var base = {
      status: "unavailable", visible: 0, hidden: 0, undefined: 0, resolution: resolution, reason: null,
      referenceMode: "terrain-surface", referencePlaneY: null, surface: null, sampleCount: resolution.width * resolution.height,
      liftM: MINIMAP_VISIBILITY_LIFT_M, epsilonM: MINIMAP_VISIBILITY_EPS_M, cells: [],
      // G34-R1 reason decomposition: `outside` is NOT `unknown`, and neither of them is `hidden`
      outsideDomain: 0, outsideView: 0, unknownData: 0, numericalDegenerate: 0, eligible: 0,
      unknownCount: 0, unresolvedCount: 0, knownCount: 0, judgedCount: 0, categorySum: 0, categoryIdentityHolds: false,
      knowledgeComplete: false, observerDegenerate: false
    };
    if (!camera || !canvasSize || !minimapWindowValid(win)) { base.reason = "missing camera, canvas size or valid window"; return base; }
    base.referencePlaneY = camera.target[1];
    var surfaceName = (typeof state !== "undefined" && state.surface) ? state.surface : null;
    base.surface = surfaceName;
    if (!model || !model.surfaces || !surfaceName || !model.surfaces[surfaceName]) { base.reason = "the model declares no surface for " + surfaceName; return base; }
    var verts = model.surfaces[surfaceName].verticesMetres;
    // G34-R2: a MISSING or NON-FINITE surface is a DATA problem, not a reason to abandon the grid.
    // The grid still runs and every cell it cannot judge is reported as unknown-data, so a surface
    // with no usable data yields partialKnowledge with unknownCount = sampleCount instead of a
    // blanket "unavailable". That is the honest answer: we are not judging anything, and we say so.
    var surfaceUsable = !!(verts && verts.length);
    // G34-R1: the footprint is a DOMAIN LIMIT, not a delete button. When the domain is empty we say
    // so, but the caller keeps drawing the (still valid) G33 footprint; the occlusion layer alone is
    // hidden. The two cases are reported with the same reason but a distinct status.
    if (!footprint || footprint.status !== "ok" || !footprint.points || footprint.points.length < 3) {
      base.status = footprint && footprint.status === "ok" ? "empty" : (footprint ? footprint.status : "unavailable");
      base.reason = footprint && footprint.reason ? footprint.reason : "the reference-plane footprint has no area inside the saved window";
      // G34-R2: nothing was sampled, so the sample count is zero and the six categories are all zero.
      // Reporting 256 here would imply cells existed and were judged, which is exactly the kind of
      // quiet overstatement this round is removing.
      base.sampleCount = 0;
      base.unknownCount = 0;
      base.judgedCount = 0;
      base.categorySum = 0;
      base.categoryIdentityHolds = true;
      base.knowledgeComplete = true;
      return base;
    }
    var index = surfaceUsable ? occlusionIndexFor(surfaceName) : null;
    // G34-R2: when the index itself cannot be built, the grid still runs with an EMPTY triangle set,
    // so every cell resolves to unknown-data and the state machine reports partialKnowledge. The
    // reason records why no triangle was available.
    var unusableReason = null;
    if (!surfaceUsable) { unusableReason = "surface " + surfaceName + " has no saved vertices, so every cell is unknown"; }
    else if (!index || !index.triangleCount) { unusableReason = "surface " + surfaceName + " produced no usable terrain triangles, so every cell is unknown"; index = null; }
    var eye = [
      camera.target[0] + camera.distance * Math.cos(camera.pitch) * Math.sin(camera.yaw),
      camera.target[1] + camera.distance * Math.sin(camera.pitch),
      camera.target[2] + camera.distance * Math.cos(camera.pitch) * Math.cos(camera.yaw)
    ];
    // G34-R2 performance instrumentation: every count below is a real measurement, published so an
    // auditor can see how much work one grid costs and how much of it was avoided. Declared BEFORE the
    // no-index branch, which also produces a cell list.
    var perf = { gridRecomputes: 1, candidateTriangles: 0, intersectionTests: 0, dedupedTriangles: 0, cellsTested: 0, cellsWithoutCandidates: 0, prefilterCleared: 0 };
    var cells = [];
    // G34-R2: the observer test uses the LOCAL interpolated terrain height under the eye, never a
    // global peak, so a viewpoint beside a hill is not called degenerate because the hill is tall.
    if (!index) {
      // no usable terrain at all: every cell is unknown, so the grid still runs and reports it
      var fpNoIndex = footprint.points;
      var sizeNoIndex = MINIMAP_SIZE;
      var stepNo = { x: win.widthM / resolution.width, z: win.depthM / resolution.height };
      for (var jn = 0; jn < resolution.height; jn++) {
        for (var inn = 0; inn < resolution.width; inn++) {
          var pxN = ((inn + 0.5) / resolution.width) * sizeNoIndex, pyN = ((jn + 0.5) / resolution.height) * sizeNoIndex;
          var wN = minimapPxToWorld(win, sizeNoIndex, pxN, pyN);
          if (!wN) { base.undefined++; base.unknownData++; cells.push({ i: inn, j: jn, klass: "undefined", category: "unknown-data", reason: "the cell centre does not map into the saved window" }); continue; }
          if (!minimapPointInPolygon(fpNoIndex, wN.x, wN.z)) { base.undefined++; base.outsideDomain++; cells.push({ i: inn, j: jn, x: wN.x, z: wN.z, klass: "undefined", category: "outside-domain", reason: "outside the footprint" }); continue; }
          base.undefined++; base.unknownData++;
          cells.push({ i: inn, j: jn, x: wN.x, z: wN.z, klass: "undefined", category: "unknown-data", reason: unusableReason });
        }
      }
      base.cells = cells;
      base.stepM = stepNo;
      base.perf = perf;
      base.unknownCount = base.unknownData;
      base.unresolvedCount = base.unknownData + base.numericalDegenerate;
      base.knownCount = base.visible + base.hidden;
      base.judgedCount = base.visible + base.hidden + base.outsideDomain + base.outsideView + base.numericalDegenerate;
      base.categorySum = base.outsideDomain + base.outsideView + base.unknownData + base.numericalDegenerate + base.visible + base.hidden;
      base.categoryIdentityHolds = base.categorySum === base.sampleCount;
      base.knowledgeComplete = base.unresolvedCount === 0 && base.categoryIdentityHolds;
      base.status = base.knownCount > 0 ? (base.hidden > 0 ? "occluded" : "unobstructed") : "unavailable";
      base.reason = base.knownCount > 0
        ? (base.hidden > 0 ? base.hidden + " of " + base.sampleCount + " cells are hidden; " + base.unresolvedCount + " unresolved" : null)
        : (unusableReason + "; no cell was decided, so this is NOT an all-clear");
      return base;
    }
    if (occlusionObserverIsDegenerate(index, eye)) {
      base.observerDegenerate = true;
      base.status = "unavailable";
      base.reason = "observer-below-or-on-terrain: the viewpoint sits at or under the locally interpolated terrain surface";
      base.observerHeightM = occlusionTerrainHeight(index, eye[0], eye[2]);
      base.eyeY = eye[1];
      base.sampleCount = 0;
      base.unknownCount = 0;
      base.judgedCount = 0;
      base.categorySum = 0;
      base.categoryIdentityHolds = true;
      base.knowledgeComplete = true;
      return base;
    }
    var fp = footprint.points;
    var size = MINIMAP_SIZE;
    var stepX = win.widthM / resolution.width, stepZ = win.depthM / resolution.height;
    // scratch state reused by the candidate de-dupe: the stamp is a COUNTER that advances for every
    // cell that actually runs an intersection test, so one stamp array serves the whole grid.
    var dedupeStamp = index.dedupeStamp, dedupeList = index.candidates;
    for (var j = 0; j < resolution.height; j++) {
      for (var i = 0; i < resolution.width; i++) {
        var px = ((i + 0.5) / resolution.width) * size, py = ((j + 0.5) / resolution.height) * size;
        var world = minimapPxToWorld(win, size, px, py);
        if (!world) { base.undefined++; base.unknownData++; cells.push({ i: i, j: j, klass: "undefined", category: "unknown-data", reason: "the cell centre does not map into the saved window" }); continue; }
        if (!minimapPointInPolygon(fp, world.x, world.z)) { base.undefined++; base.outsideDomain++; cells.push({ i: i, j: j, x: world.x, z: world.z, klass: "undefined", category: "outside-domain", reason: "outside the footprint" }); continue; }
        var height = index ? occlusionTerrainHeight(index, world.x, world.z) : null;
        if (height === null) {
          base.undefined++; base.unknownData++;
          var why = index ? "no usable terrain height at this cell" : unusableReason;
          cells.push({ i: i, j: j, x: world.x, z: world.z, klass: "undefined", category: "unknown-data", reason: why });
          continue;
        }
        // inside the domain and the terrain is known: this cell is eligible for a judgement
        if (!occlusionInsideFrustum(camera, canvasSize, world, height + MINIMAP_VISIBILITY_LIFT_M)) {
          base.undefined++; base.outsideView++;
          cells.push({ i: i, j: j, x: world.x, z: world.z, referenceY: height, klass: "undefined", category: "outside-view", reason: "the surface point is outside the near/far frustum of the current camera" });
          continue;
        }
        base.eligible++;
        perf.cellsTested++;
        // one unique epoch per cell, from the monotonic source shared by every user of the array
        var cellEpoch = nextOcclusionEpoch(dedupeStamp);
        var detail = occlusionClassifyDetailed(index, eye, world.x, world.z, height, perf, cellEpoch, dedupeStamp, dedupeList);
        if (detail.candidateCount === 0) { perf.cellsWithoutCandidates++; }
        if (detail.klass === "hidden") { base.hidden++; cells.push({ i: i, j: j, x: world.x, z: world.z, referenceY: height, klass: "hidden", category: "hidden", reason: detail.reason, sMetres: detail.sMetres, candidates: detail.candidateCount, tested: detail.testedCount, deduped: detail.dedupedCount }); }
        else if (detail.klass === "visible") { base.visible++; cells.push({ i: i, j: j, x: world.x, z: world.z, referenceY: height, klass: "visible", category: "visible", reason: detail.reason, sMetres: detail.sMetres, candidates: detail.candidateCount, tested: detail.testedCount, deduped: detail.dedupedCount }); }
        else { base.undefined++; base.numericalDegenerate++; cells.push({ i: i, j: j, x: world.x, z: world.z, referenceY: height, klass: "undefined", category: "numerical-degenerate", reason: detail.reason }); }
      }
    }
    base.cells = cells;
    base.stepM = { x: stepX, z: stepZ };
    base.perf = perf;
    // ---------------------------------------------------------------------------------------------
    // G34-R2 state contract. The SIX categories are mutually exclusive and must sum to sampleCount:
    //   outsideDomain + outsideView + unknownData + numericalDegenerate + visible + hidden == 256
    // `unknownCount` is its OWN quantity (genuinely missing data). It is deliberately NOT the
    // `undefined` total: cells outside the domain or outside the view have been JUDGED, they simply
    // are not visible from this camera. The earlier revision confused the two and reported a domain
    // that was half outside the footprint as "partial knowledge", which the inspector caught.
    // ---------------------------------------------------------------------------------------------
    // ---------------------------------------------------------------------------------------------
    // G34-R3 state contract. The six categories stay mutually exclusive and sum to sampleCount.
    // Missing data and numerical degeneracy are counted SEPARATELY but both are UNRESOLVED: neither
    // has produced a judgement, so `unresolvedCount = unknownData + numericalDegenerate` is the
    // quantity that governs completeness. `knowledgeComplete` can therefore never be true while any
    // numerical-degenerate cell exists -- the previous revision could report "unobstructed,
    // knowledgeComplete true" for a grid where EVERY cell was numerically degenerate, which is the
    // clearest possible false all-clear.
    // ---------------------------------------------------------------------------------------------
    base.unknownCount = base.unknownData;
    base.unresolvedCount = base.unknownData + base.numericalDegenerate;
    base.judgedCount = base.visible + base.hidden + base.outsideDomain + base.outsideView + base.numericalDegenerate;
    base.categorySum = base.outsideDomain + base.outsideView + base.unknownData + base.numericalDegenerate + base.visible + base.hidden;
    base.categoryIdentityHolds = base.categorySum === base.sampleCount;
    base.knowledgeComplete = base.unresolvedCount === 0;
    base.knownCount = base.visible + base.hidden;
    if (!base.categoryIdentityHolds) {
      // an internal contradiction must be loud, never silently rounded away
      base.status = "unavailable";
      base.reason = "internal error: the six mutually exclusive categories sum to " + base.categorySum + " instead of " + base.sampleCount;
      return base;
    }
    if (base.hidden > 0) {
      // an occluded result is a positive fact, so it outranks unresolved cells -- but the unresolved
      // count must be visible, and the hidden count is stated as the LOWER BOUND it is
      base.status = "occluded";
      base.reason = base.hidden + " of " + base.sampleCount + " cells are hidden by the saved terrain"
        + (base.unresolvedCount > 0 ? "; " + base.unresolvedCount + " more cells are unresolved (unknown-data " + base.unknownData + ", numerical-degenerate " + base.numericalDegenerate + "), so the hidden count is a LOWER BOUND" : "");
    } else if (base.knownCount === 0) {
      // nothing at all was decided: this must never be dressed up as an all-clear
      base.status = "unavailable";
      base.reason = "no cell was decided: " + base.unresolvedCount + " unresolved (unknown-data " + base.unknownData + ", numerical-degenerate " + base.numericalDegenerate + "), " + base.outsideDomain + " outside the domain, " + base.outsideView + " outside the view";
    } else if (base.unresolvedCount > 0) {
      base.status = "partialKnowledge";
      base.reason = "no known cell is hidden, but " + base.unresolvedCount + " of " + base.sampleCount + " cells are unresolved (unknown-data " + base.unknownData + ", numerical-degenerate " + base.numericalDegenerate + "), so this is NOT a global all-clear";
    } else if (base.outsideView > 0 && base.knownCount === 0) {
      base.status = "unavailable";
      base.reason = "no eligible cell: every footprint cell lies outside the current view frustum";
    } else if (base.knownCount > 0) {
      // every known cell is visible and nothing is unresolved or hidden. Cells outside the domain or
      // outside the view have been JUDGED and are not unknown, so they do not downgrade this result.
      base.status = "unobstructed";
      base.reason = null;
    } else {
      base.status = "unavailable";
      base.reason = "no eligible cell inside the sampled domain";
    }
    return base;
  }
  /// Click/drag on the overview moves ONLY the horizontal target, exactly like the
  /// G32 pan. Interactive input is CLAMPED into the saved window in user units, while the
  /// projection itself is never truncated. Returns null (camera untouched) for degenerate input.
  function minimapTargetFromPx(win, size, px, py, startTarget) {    if (!startTarget || !Array.isArray(startTarget) || startTarget.length !== 3) { return null; }
    for (var i = 0; i < 3; i++) { if (typeof startTarget[i] !== "number" || !isFinite(startTarget[i])) { return null; } }
    if (typeof px !== "number" || !isFinite(px) || typeof py !== "number" || !isFinite(py)) { return null; }
    var world = minimapPxToWorld(win, size, clamp(px, 0, size), clamp(py, 0, size));
    if (!world) { return null; }
    return [world.x, startTarget[1], world.z];
  }
  function applyMinimapJump(px, py, cause) {
    if (!model || !model.window) { return false; }
    var target = minimapTargetFromPx(model.window, MINIMAP_SIZE, px, py, state.camera.target);
    if (!target) { return false; }
    state.camera.target = target;
    state.camera.mode = "free";
    state.cameraMoves++;
    state.minimapMoves++;
    if (cause === "pointerup") { state.minimapJumps++; }
    state.lastMinimap = { cause: cause, px: round6(px), py: round6(py), to: [round6(target[0]), round6(target[1]), round6(target[2])] };
    render();
    return true;
  }
  /// One exit for every minimap cancel path, mirroring endPointerGesture.
  function endMinimapDrag(reason) {
    if (!state.minimapDrag) { return null; }
    var drag = state.minimapDrag;
    state.minimapDrag = null;
    drag.endReason = reason;
    state.minimapDragLog.push({ endReason: reason, moved: round6(drag.moved), applied: drag.applied === true });
    if (state.minimapDragLog.length > 16) { state.minimapDragLog.shift(); }
    return drag;
  }
  function minimapPointFromEvent(element, event) {
    var rect = element.getBoundingClientRect();
    var w = rect.width || MINIMAP_SIZE, h = rect.height || MINIMAP_SIZE;
    return { px: ((event.clientX - rect.left) / w) * MINIMAP_SIZE, py: ((event.clientY - rect.top) / h) * MINIMAP_SIZE };
  }
  function drawMinimap() {
    var svg = document.getElementById("minimap");
    if (!svg || !model || !model.window) { return null; }
    var win = model.window, size = MINIMAP_SIZE, camera = state.camera;
    var canvasEl = document.getElementById("gl");
    var canvasSize = canvasEl ? canvasCssSize(canvasEl) : null;
    var footprint = canvasSize ? minimapFootprint(camera, canvasSize, win, camera.target[1]) : null;
    var poly = document.getElementById("minimap-footprint");
    if (poly) {
      var drawn = null;
      if (footprint && footprint.status === "ok" && footprint.points.length >= 3) {
        drawn = [];
        for (var i = 0; i < footprint.points.length; i++) {
          var p = minimapWorldToPx(win, size, footprint.points[i][0], footprint.points[i][1]);
          if (!p) { drawn = null; break; }
          drawn.push(round6(clamp(p.px, 0, size)) + "," + round6(clamp(p.py, 0, size)));
        }
      }
      if (drawn && drawn.length >= 3) { poly.setAttribute("points", drawn.join(" ")); poly.setAttribute("display", "inline"); }
      else { poly.setAttribute("points", ""); poly.setAttribute("display", "none"); }
    }
    var targetPx = minimapWorldToPx(win, size, camera.target[0], camera.target[2]);
    var dot = document.getElementById("minimap-target");
    if (dot && targetPx) { dot.setAttribute("cx", String(round6(clamp(targetPx.px, 0, size)))); dot.setAttribute("cy", String(round6(clamp(targetPx.py, 0, size)))); }
    // G34: the occlusion layer. Only HIDDEN cells are drawn; visible and undefined cells are not,
    // so an unobstructed view leaves the layer absent instead of painting a see-through sheet.
    // G34-R1: the whole group is clipped to the window rectangle by a real SVG clipPath, so a cell
    // whose CENTRE is inside the footprint can never paint colour outside the sampled domain.
    var visibility = minimapVisibilityGrid(camera, canvasSize, win, footprint);
    var hiddenGroup = document.getElementById("minimap-hidden");
    if (hiddenGroup) {
      while (hiddenGroup.firstChild) { hiddenGroup.removeChild(hiddenGroup.firstChild); }
      var drawnHidden = 0;
      var drawnCellIds = [];
      var stepXpx = visibility.stepM ? (visibility.stepM.x / win.widthM) * size : 0;
      var stepZpx = visibility.stepM ? (visibility.stepM.z / win.depthM) * size : 0;
      // G34-R3: the clip is TWO NESTED groups, each with its own SINGLE-shape clipPath. A clipPath
      // holding a polygon and a rect together is their UNION, which is what the previous revision
      // shipped; nesting gives the intersection cellRect ∩ footprint ∩ window without any union.
      // The half-cell inset that had no contract basis is GONE: the window clip is the window.
      var clipPoly = document.getElementById("minimap-domain-clip-poly");
      var clipWindow = document.getElementById("minimap-domain-clip-window");
      var clipGroup = document.getElementById("minimap-footprint-clip-group");
      var clipPoints = 0;
      // captured for the atomic snapshot below, in the same frame the rects are written
      var clipWindowRect = null;
      if (clipPoly) {
        var fpz = (footprint && footprint.points) ? footprint.points : null;
        if (fpz && fpz.length >= 3) {
          var mapped = [];
          for (var ci2 = 0; ci2 < fpz.length; ci2++) {
            var mp = minimapWorldToPx(win, size, fpz[ci2][0], fpz[ci2][1]);
            if (!mp) { mapped = null; break; }
            mapped.push(round6(mp.px) + "," + round6(mp.py));
          }
          if (mapped && mapped.length >= 3) { clipPoly.setAttribute("points", mapped.join(" ")); clipPoints = mapped.length; }
          else { clipPoly.setAttribute("points", ""); }
        } else { clipPoly.setAttribute("points", ""); }
      }
      if (clipWindow) {
        // the sampled domain's own rectangle, in the same 140x140 user units, with no inset
        var wl = minimapWorldToPx(win, size, win.minX, win.minZ);
        var wr = minimapWorldToPx(win, size, win.minX + win.widthM, win.minZ + win.depthM);
        if (wl && wr) {
          clipWindow.setAttribute("x", String(round6(Math.min(wl.px, wr.px))));
          clipWindow.setAttribute("y", String(round6(Math.min(wl.py, wr.py))));
          clipWindow.setAttribute("width", String(round6(Math.abs(wr.px - wl.px))));
          clipWindow.setAttribute("height", String(round6(Math.abs(wr.py - wl.py))));
          clipWindowRect = {
            x: round6(Math.min(wl.px, wr.px)), y: round6(Math.min(wl.py, wr.py)),
            width: round6(Math.abs(wr.px - wl.px)), height: round6(Math.abs(wr.py - wl.py))
          };
        }
      }
      // the outer clip group is visible only while there is a polygon to clip to; an empty domain must
      // leave no residual rect painted
      if (clipGroup) { clipGroup.setAttribute("display", clipPoints >= 3 && visibility.hidden > 0 ? "inline" : "none"); }
      if (visibility.hidden > 0 && visibility.cells && visibility.cells.length) {
        for (var ci = 0; ci < visibility.cells.length; ci++) {
          var cell = visibility.cells[ci];
          if (cell.klass !== "hidden") { continue; }
          var cp = minimapWorldToPx(win, size, cell.x, cell.z);
          if (!cp) { continue; }
          var rect = document.createElementNS("http://www.w3.org/2000/svg", "rect");
          // the drawn unit is the WHOLE display cell, centred on the sampled centre; the clip above
          // trims whatever part of it leaves the domain
          rect.setAttribute("x", String(round6(cp.px - stepXpx / 2)));
          rect.setAttribute("y", String(round6(cp.py - stepZpx / 2)));
          rect.setAttribute("width", String(round6(stepXpx)));
          rect.setAttribute("height", String(round6(stepZpx)));
          rect.setAttribute("class", "minimap-hidden-cell");
          // per-cell identity so a reviewer can compare the DRAWING with the oracle cell by cell
          rect.setAttribute("data-cell", cell.i + ":" + cell.j);
          rect.setAttribute("data-class", cell.klass);
          rect.setAttribute("data-category", cell.category || (cell.klass === "hidden" ? "hidden" : "unknown-data"));
          rect.setAttribute("data-s-metres", cell.sMetres === null || cell.sMetres === undefined ? "" : String(round6(cell.sMetres)));
          hiddenGroup.appendChild(rect);
          drawnCellIds.push(cell.i + ":" + cell.j);
          drawnHidden++;
        }
      }
      if (drawnHidden > 0) { hiddenGroup.setAttribute("display", "inline"); }
      else { hiddenGroup.setAttribute("display", "none"); }
      hiddenGroup.setAttribute("data-hidden-cells", String(drawnHidden));
    }
    // ---------------------------------------------------------------------------------------------
    // G34-R4 ATOMIC VISIBILITY SNAPSHOT.
    //
    // Everything a reader can quote about "this frame" comes from ONE object built here, from ONE
    // grid evaluation, with the DOM write above already performed. The count fields, the full cell
    // list and the set of cells actually painted are therefore the SAME frame by construction: it is
    // no longer possible to quote a count from one call and a cell list from another and call the
    // difference a contradiction.
    //
    // The object and every cell record inside it are FROZEN. Reading it cannot mutate it, and a later
    // render cannot rewrite it -- a stale reference keeps showing the frame it came from.
    // ---------------------------------------------------------------------------------------------
    var snapCells = [];
    for (var sci = 0; sci < visibility.cells.length; sci++) {
      var sc = visibility.cells[sci];
      snapCells.push(Object.freeze({
        i: sc.i, j: sc.j, x: round6(sc.x), z: round6(sc.z),
        category: sc.category || null, klass: sc.klass,
        reason: sc.reason === undefined ? null : sc.reason,
        sMetres: sc.sMetres === null || sc.sMetres === undefined ? null : round6(sc.sMetres),
        referenceY: sc.referenceY === undefined || sc.referenceY === null ? null : round6(sc.referenceY)
      }));
    }
    // the consistency the snapshot claims about itself, computed here so a contradiction is recorded
    // rather than silently displayed
    var snapHiddenCount = 0, snapVisibleCount = 0, snapOutsideDomain = 0, snapOutsideView = 0, snapUnknown = 0, snapDegenerate = 0;
    for (var sck = 0; sck < snapCells.length; sck++) {
      var kk = snapCells[sck].klass;
      var cc = snapCells[sck].category;
      if (kk === "hidden") { snapHiddenCount++; }
      else if (kk === "visible") { snapVisibleCount++; }
      if (cc === "outside-domain") { snapOutsideDomain++; }
      else if (cc === "outside-view") { snapOutsideView++; }
      else if (cc === "unknown-data") { snapUnknown++; }
      else if (cc === "numerical-degenerate") { snapDegenerate++; }
    }
    var snapCategorySum = snapOutsideDomain + snapOutsideView + snapUnknown + snapDegenerate + snapVisibleCount + snapHiddenCount;
    visibilitySnapshotSerial++;
    var drawnSorted = (typeof drawnCellIds === "undefined" ? [] : drawnCellIds.slice()).sort();
    var hiddenSorted = snapCells.filter(function (c) { return c.klass === "hidden"; }).map(function (c) { return c.i + ":" + c.j; }).sort();
    var snapshotConsistency = {
      cellsCounted: snapCells.length,
      selfConsistent: true,
      hiddenEqualsCells: snapHiddenCount === visibility.hidden,
      visibleEqualsCells: snapVisibleCount === visibility.visible,
      outsideDomainEqualsCells: snapOutsideDomain === visibility.outsideDomain,
      outsideViewEqualsCells: snapOutsideView === visibility.outsideView,
      unknownDataEqualsCells: snapUnknown === visibility.unknownData,
      numericalDegenerateEqualsCells: snapDegenerate === visibility.numericalDegenerate,
      categorySumEqualsSampleCount: snapCategorySum === visibility.sampleCount,
      unresolvedIsSumOfUnresolved: visibility.unresolvedCount === visibility.unknownData + visibility.numericalDegenerate,
      knownCountIsVisiblePlusHidden: visibility.knownCount === visibility.visible + visibility.hidden,
      knowledgeCompleteMatchesUnresolved: visibility.knowledgeComplete === (visibility.unresolvedCount === 0),
      drawnEqualsHiddenCells: drawnSorted.join(",") === hiddenSorted.join(",")
    };
    // any disagreement makes the whole snapshot self-report as inconsistent, so a contradiction is
    // recorded in the artefact rather than only being visible to whoever reads it closely
    for (var snk in snapshotConsistency) {
      if (snk !== "cellsCounted" && snapshotConsistency[snk] !== true) { snapshotConsistency.selfConsistent = false; }
    }
    var visibilitySnapshot = Object.freeze({
      schema: "g34-visibility-snapshot-v1",
      snapshotId: visibilitySnapshotSerial,
      // the exact inputs, so the same frame can be reproduced without guessing
      camera: Object.freeze({ mode: camera.mode, yaw: round6(camera.yaw), pitch: round6(camera.pitch), distance: round6(camera.distance), target: Object.freeze([round6(camera.target[0]), round6(camera.target[1]), round6(camera.target[2])]), minDistance: round6(camera.minDistance), maxDistance: round6(camera.maxDistance) }),
      canvas: Object.freeze({
        cssWidth: canvasSize ? round6(canvasSize.width) : null,
        cssHeight: canvasSize ? round6(canvasSize.height) : null,
        backingWidth: canvasEl ? canvasEl.width : null,
        backingHeight: canvasEl ? canvasEl.height : null,
        dpr: window.devicePixelRatio === undefined ? null : window.devicePixelRatio
      }),
      window: Object.freeze({ minX: win.minX, minZ: win.minZ, widthM: win.widthM, depthM: win.depthM }),
      footprint: Object.freeze({ status: footprint ? footprint.status : null, points: footprint && footprint.points ? footprint.points.length : 0, hash: visibilityFootprintHash(footprint) }),
      surface: visibility.surface === null ? null : visibility.surface,
      resolution: Object.freeze({ width: visibility.resolution.width, height: visibility.resolution.height }),
      liftM: visibility.liftM, epsilonM: visibility.epsilonM,
      fov: visibility.fov === undefined ? null : visibility.fov,
      nearM: visibility.nearM === undefined ? null : visibility.nearM,
      farM: visibility.farM === undefined ? null : visibility.farM,
      cells: Object.freeze(snapCells),
      // aggregates, published exactly as the cells above were counted
      hidden: visibility.hidden, visible: visibility.visible,
      outsideDomain: visibility.outsideDomain, outsideView: visibility.outsideView,
      unknownData: visibility.unknownData, numericalDegenerate: visibility.numericalDegenerate,
      unknownCount: visibility.unknownCount, unresolvedCount: visibility.unresolvedCount,
      knownCount: visibility.knownCount, status: visibility.status, reason: visibility.reason,
      categorySum: snapCategorySum, sampleCount: visibility.sampleCount,
      knowledgeComplete: visibility.knowledgeComplete === true,
      // what was ACTUALLY painted, in the same frame
      drawnCells: Object.freeze(drawnSorted),
      clip: Object.freeze({
        footprintPoints: clipPoints,
        windowRect: clipWindowRect,
        structure: "two-nested-single-shape-clipPaths"
      }),
      consistency: Object.freeze(snapshotConsistency),
      perf: visibility.perf ? Object.freeze({
        gridRecomputes: visibility.perf.gridRecomputes, candidateTriangles: visibility.perf.candidateTriangles,
        intersectionTests: visibility.perf.intersectionTests, dedupedTriangles: visibility.perf.dedupedTriangles,
        cellsTested: visibility.perf.cellsTested, prefilterCleared: visibility.perf.prefilterCleared || 0
      }) : null
    });
    state.minimapVisibility = visibilitySnapshot;
    var statusEl = document.getElementById("minimap-visibility-status");
    if (statusEl) {
      // G34-R1: the SHORT status must be visible inside the minimap box and must state the sampling
      // restriction in words, not only in data attributes. The full limitation text lives in the
      // title/aria attributes and in the existing inspector panel; no floating layer is added.
      var kind = {
        occluded: "已遮挡",
        unobstructed: "通视",
        partialKnowledge: "部分未知",
        empty: "域为空",
        unavailable: "不可判定"
      }[visibility.status] || visibility.status;
      // G34-R3: the visible numbers must be the SAME numbers the snapshot publishes. The previous
      // revision printed the `undefined` total (which also contains outside-domain and outside-view
      // cells that WERE judged) and called it "unknown", so a grid with 16 missing cells and 120
      // judged-but-outside cells displayed "136 unknown". Every figure below is now read from the
      // matching published field.
      var detail;
      if (visibility.status === "occluded") {
        detail = visibility.hidden + "/" + visibility.sampleCount + " 格被地形挡住" + (visibility.unresolvedCount > 0 ? "（下界；另有 " + visibility.unresolvedCount + " 格未决）" : "");
      } else if (visibility.status === "unobstructed") {
        detail = "已知 " + visibility.knownCount + " 格全部通视（域外 " + visibility.outsideDomain + "、视锥外 " + visibility.outsideView + "）";
      } else if (visibility.status === "partialKnowledge") {
        detail = "0 格被挡，未决 " + visibility.unresolvedCount + " 格（缺数据 " + visibility.unknownData + "、数值退化 " + visibility.numericalDegenerate + "）";
      } else if (visibility.status === "empty") {
        detail = "采样域为空";
      } else {
        detail = "未决 " + visibility.unresolvedCount + " 格，已知 " + visibility.knownCount + " 格";
      }
      statusEl.textContent = "地表采样·仅地形·" + visibility.resolution.width + "×" + visibility.resolution.height + " — " + kind + "：" + detail;
      statusEl.setAttribute("data-status", visibility.status);
      statusEl.setAttribute("data-visible", String(visibility.visible));
      statusEl.setAttribute("data-hidden", String(visibility.hidden));
      statusEl.setAttribute("data-undefined", String(visibility.undefined));
      statusEl.setAttribute("data-outside-domain", String(visibility.outsideDomain));
      statusEl.setAttribute("data-outside-view", String(visibility.outsideView));
      statusEl.setAttribute("data-unknown-data", String(visibility.unknownData));
      statusEl.setAttribute("data-numerical-degenerate", String(visibility.numericalDegenerate));
      statusEl.setAttribute("data-eligible", String(visibility.eligible));
      statusEl.setAttribute("data-known-count", String(visibility.knownCount));
      statusEl.setAttribute("data-unresolved-count", String(visibility.unresolvedCount));
      statusEl.setAttribute("data-knowledge-complete", String(visibility.knowledgeComplete === true));
      statusEl.setAttribute("data-resolution", visibility.resolution.width + "x" + visibility.resolution.height);
      statusEl.setAttribute("data-reason", visibility.reason === null ? "" : String(visibility.reason));
      statusEl.setAttribute("data-reference-mode", visibility.referenceMode);
      statusEl.setAttribute("data-clip-polygon-points", String(clipPoints));
      statusEl.setAttribute("data-unknown-count", String(visibility.unknownCount));
      statusEl.setAttribute("data-category-sum", String(visibility.categorySum));
      statusEl.setAttribute("data-category-identity", String(visibility.categoryIdentityHolds === true));
      statusEl.setAttribute("data-grid-recomputes", String(visibility.perf ? visibility.perf.gridRecomputes : 0));
      statusEl.setAttribute("data-intersection-tests", String(visibility.perf ? visibility.perf.intersectionTests : 0));
      statusEl.setAttribute("data-deduped-triangles", String(visibility.perf ? visibility.perf.dedupedTriangles : 0));
      statusEl.setAttribute("data-surface", visibility.surface === null ? "" : String(visibility.surface));
      statusEl.setAttribute("title", "小地图地形通视采样：在 G33 采样域内，对当前 surface 的地表采样点（保存三角插值高度 + " + visibility.liftM + " m 抬升）与当前观察点之间做地形线段求交。仅保存地形参与判定，实体/道路不计；域外、视锥外与数据缺失分别计为 outside-domain / outside-view / unknown-data，都不等于 hidden，也不等于 visible。res=" + visibility.resolution.width + "×" + visibility.resolution.height + "，epsilon=" + visibility.epsilonM + " m，surface=" + visibility.surface + "。");
      statusEl.setAttribute("aria-label", "地形通视采样结果：" + kind + "。" + detail + "。仅地形，采样 " + visibility.resolution.width + "×" + visibility.resolution.height + "。");
      statusEl.setAttribute("display", "inline");
    }
    state.minimap = {
      size: size,
      footprintAvailable: !!(footprint && footprint.status === "ok" && footprint.points.length >= 3),
      footprintStatus: footprint ? footprint.status : "unavailable",
      footprintReason: footprint && footprint.reason ? footprint.reason : null,
      footprintPoints: footprint && footprint.points ? footprint.points.length : 0,
      footprintClippedToWindow: footprint ? footprint.clippedToWindow === true : false,
      footprintFrustumInsideWindow: footprint && footprint.frustumInsideWindow !== undefined ? footprint.frustumInsideWindow === true : null,
      target: targetPx ? { px: round6(targetPx.px), py: round6(targetPx.py) } : null,
      targetWorld: [round6(camera.target[0]), round6(camera.target[1]), round6(camera.target[2])],
      windowBox: { minX: win.minX, minZ: win.minZ, widthM: win.widthM, depthM: win.depthM },
      canvasCss: canvasSize ? { width: round6(canvasSize.width), height: round6(canvasSize.height) } : null,
      visibilityStatus: visibility.status,
      visibilityVisible: visibility.visible,
      visibilityHidden: visibility.hidden,
      visibilityUndefined: visibility.undefined,
      visibilityResolution: { width: visibility.resolution.width, height: visibility.resolution.height },
      visibilityReason: visibility.reason,
      visibilityOutsideDomain: visibility.outsideDomain,
      visibilityOutsideView: visibility.outsideView,
      visibilityUnknownData: visibility.unknownData,
      visibilityNumericalDegenerate: visibility.numericalDegenerate,
      visibilityEligible: visibility.eligible,
      visibilityKnowledgeComplete: visibility.knowledgeComplete === true,
      moves: state.minimapMoves, jumps: state.minimapJumps,
      dragActive: state.minimapDrag !== null
    };
    return state.minimap;
  }
  function zoomCamera(delta) {
    var c = state.camera;
    c.distance = clamp(c.distance * Math.exp(delta * 0.0012), c.minDistance || 1, c.maxDistance || 1e6);
    c.mode = "free";
    state.cameraMoves++;
    render();
  }
  // ------------------------------------------------------------------ G29 modes
  /// The browse/inspect switch is a layout change only: it never reloads the
  /// model, never builds a second scene and never touches the camera, the chosen
  /// surface or the selection. It moves the facts panel in or out of the flex row
  /// and lets the canvas follow the new rectangle, so nothing overlays the map.
  function applyViewMode() {
    var inspect = state.viewMode === "inspect";
    if (document.body) { document.body.className = inspect ? "view-inspect" : "view-browse hud-collapsed"; }
    var browseButton = document.getElementById("mode-browse");
    if (browseButton) { browseButton.setAttribute("aria-pressed", inspect ? "false" : "true"); }
    var inspectButton = document.getElementById("mode-inspect");
    if (inspectButton) { inspectButton.setAttribute("aria-pressed", inspect ? "true" : "false"); }
    var panelToggle = document.getElementById("hud-toggle");
    if (panelToggle) {
      panelToggle.textContent = inspect ? "收起检查信息" : "显示检查信息";
      panelToggle.setAttribute("aria-expanded", inspect ? "true" : "false");
    }
    state.hudCollapsed = !inspect;
  }
  /// The map rectangle measured from the real layout boxes. The acceptance
  /// driver measures the same boxes independently; this copy exists so the page
  /// state and the DOM can be compared instead of one being trusted.
  function measureLayout() {
    var canvas = document.getElementById("gl");
    var stage = document.getElementById("stage");
    var hud = document.getElementById("hud");
    var doc = document.documentElement || {};
    var viewportWidth = window.innerWidth || doc.clientWidth || 0;
    var viewportHeight = window.innerHeight || doc.clientHeight || 0;
    var stageRect = stage ? stage.getBoundingClientRect() : null;
    var canvasRect = canvas ? canvas.getBoundingClientRect() : null;
    var mapWidth = Math.round(canvasRect ? canvasRect.width : 0);
    var mapHeight = Math.round(canvasRect ? canvasRect.height : 0);
    state.layout = {
      viewMode: state.viewMode,
      viewportWidth: viewportWidth, viewportHeight: viewportHeight,
      mapWidth: mapWidth, mapHeight: mapHeight,
      mapWidthFraction: viewportWidth > 0 ? round6(mapWidth / viewportWidth) : 0,
      mapHeightFraction: viewportHeight > 0 ? round6(mapHeight / viewportHeight) : 0,
      stageWidth: Math.round(stageRect ? stageRect.width : 0),
      canvasWidth: canvas ? canvas.width : 0, canvasHeight: canvas ? canvas.height : 0,
      factsPanelVisible: !!(hud && hud.offsetWidth > 0 && hud.offsetHeight > 0)
    };
    return state.layout;
  }
  function viewModeSnapshot() {
    return {
      viewMode: state.viewMode, modeSwitches: state.modeSwitches, layoutSyncs: state.layoutSyncs,
      hudCollapsed: state.hudCollapsed, selectionDetailsOpen: state.selectionDetailsOpen,
      surface: state.surface, selectionObjectKey: state.selection ? state.selection.objectKey : null,
      selectedEntityId: state.selectedEntityId, camera: cameraSnapshot(), layout: state.layout
    };
  }
  function syncLayout() {
    applyViewMode();
    resizeCanvas();
    state.layoutSyncs++;
    render();
    renderSelection();
    measureLayout();
    return viewModeSnapshot();
  }
  function setViewMode(mode) {
    var next = (mode === "inspect") ? "inspect" : "browse";
    if (state.viewMode !== next) { state.viewMode = next; state.modeSwitches++; }
    return syncLayout();
  }
  /// Kept from the G28 contract so existing bindings and records still hold; the
  /// collapsed facts panel is exactly browse mode.
  function setHudCollapsed(collapsed) { return setViewMode(collapsed ? "browse" : "inspect"); }

  // ------------------------------------------------------------------ drawing
  function upload(name, data) {
    if (!buffers[name]) { buffers[name] = gl.createBuffer(); }
    gl.bindBuffer(gl.ARRAY_BUFFER, buffers[name]);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(data), gl.STATIC_DRAW);
    return buffers[name];
  }
  // --------------------------------------------------------------- topology
  // G28-R1: one validated topology function owns the terrain index grid. It
  // rejects illegal dimensions explicitly instead of silently returning an empty
  // array (the G28 regression passed the surface object as the width, which
  // produced NaN loop bounds, zero indices, a wrong-topology drawArrays fallback
  // and an empty pick set).
  function terrainTopology(width, depth, vertexCount) {
    if (typeof width !== "number" || typeof depth !== "number" || !isFinite(width) || !isFinite(depth)) {
      throw new Error("terrain topology needs numeric width and depth, received " + describe(width) + " and " + describe(depth));
    }
    if (!Number.isInteger(width) || !Number.isInteger(depth)) {
      throw new Error("terrain topology needs integer width and depth, received " + width + " x " + depth);
    }
    if (width < 2 || depth < 2) {
      throw new Error("terrain topology needs width and depth of at least 2, received " + width + " x " + depth);
    }
    if (vertexCount !== undefined) {
      if (!Number.isInteger(vertexCount)) {
        throw new Error("terrain topology needs an integer vertex count, received " + describe(vertexCount));
      }
      if (vertexCount !== width * depth) {
        throw new Error("terrain topology " + width + " x " + depth + " describes " + (width * depth) + " vertices but the surface carries " + vertexCount);
      }
    }
    var indices = [];
    for (var z = 0; z < depth - 1; z++) {
      for (var x = 0; x < width - 1; x++) {
        var a = z * width + x, b = a + 1, c = a + width, d = c + 1;
        indices.push(a, c, b, b, c, d);
      }
    }
    return indices;
  }
  function describe(value) {
    if (value === null) { return "null"; }
    if (typeof value === "object") { return "an object"; }
    return String(value);
  }
  /// The terrain indices of one named surface, read from the model's grid and
  /// cross-checked against the surface's own vertex and triangle declarations.
  /// Export and viewer therefore cannot drift apart silently.
  function terrainIndicesFor(name) {
    var grid = model.grid ? model.grid[name] : null;
    if (!grid) { throw new Error("the model declares no grid for surface " + name); }
    var surface = model.surfaces ? model.surfaces[name] : null;
    if (!surface) { throw new Error("the model declares no surface " + name); }
    var declaredVertices = grid.vertexCount;
    if (declaredVertices !== undefined && declaredVertices !== surface.vertexCount) {
      throw new Error("surface " + name + " has " + surface.vertexCount + " vertices but its grid declares " + declaredVertices);
    }
    var indices = terrainTopology(grid.sampleWidth, grid.sampleDepth, surface.vertexCount);
    var triangles = indices.length / 3;
    if (surface.triangles !== undefined && triangles !== surface.triangles) {
      throw new Error("surface " + name + " declares " + surface.triangles + " triangles but its grid produces " + triangles);
    }
    if (!indices.length) { throw new Error("surface " + name + " produced no terrain indices"); }
    return indices;
  }
  function drawIndexedTriangles(vertices, colours, indices, label) {
    if (!indices || !indices.length) {
      throw new Error("the indexed terrain draw path requires a non-empty index buffer (" + label + ")");
    }
    var position = upload("pos", vertices), colour = upload("col", colours);
    gl.bindBuffer(gl.ARRAY_BUFFER, position);
    var aPos = gl.getAttribLocation(program, "aPos");
    gl.enableVertexAttribArray(aPos); gl.vertexAttribPointer(aPos, 3, gl.FLOAT, false, 0, 0);
    gl.bindBuffer(gl.ARRAY_BUFFER, colour);
    var aColor = gl.getAttribLocation(program, "aColor");
    gl.enableVertexAttribArray(aColor); gl.vertexAttribPointer(aColor, 3, gl.FLOAT, false, 0, 0);
    if (!buffers.index) { buffers.index = gl.createBuffer(); }
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, buffers.index);
    gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, new Uint32Array(indices), gl.STATIC_DRAW);
    gl.drawElements(gl.TRIANGLES, indices.length, gl.UNSIGNED_INT, 0);
    state.drawCalls++;
    state.indexedDrawCalls++;
    state.submittedIndices += indices.length;
  }
  // Entity and road solids are flat triangle soups built per face, so they use
  // the explicit non-indexed path. The terrain never uses it.
  function drawSolidTriangles(vertices, colours) {
    if (!vertices || !vertices.length) { throw new Error("the solid draw path requires vertices"); }
    var position = upload("pos", vertices), colour = upload("col", colours);
    gl.bindBuffer(gl.ARRAY_BUFFER, position);
    var aPos = gl.getAttribLocation(program, "aPos");
    gl.enableVertexAttribArray(aPos); gl.vertexAttribPointer(aPos, 3, gl.FLOAT, false, 0, 0);
    gl.bindBuffer(gl.ARRAY_BUFFER, colour);
    var aColor = gl.getAttribLocation(program, "aColor");
    gl.enableVertexAttribArray(aColor); gl.vertexAttribPointer(aColor, 3, gl.FLOAT, false, 0, 0);
    gl.drawArrays(gl.TRIANGLES, 0, vertices.length / 3);
    state.drawCalls++;
    state.solidDrawCalls++;
  }
  function heightRange(surface) {
    var values = surface.heightsMm, min = Infinity, max = -Infinity;
    for (var i = 0; i < values.length; i++) { if (values[i] < min) { min = values[i]; } if (values[i] > max) { max = values[i]; } }
    return { min: min, max: max };
  }
  function rampColour(ramp, t) {
    var scaled = clamp(t, 0, 1) * (ramp.length - 1);
    var index = Math.min(ramp.length - 2, Math.floor(scaled));
    var f = scaled - index;
    return [
      ramp[index][0] + (ramp[index + 1][0] - ramp[index][0]) * f,
      ramp[index][1] + (ramp[index + 1][1] - ramp[index][1]) * f,
      ramp[index][2] + (ramp[index + 1][2] - ramp[index][2]) * f
    ];
  }
  function surfaceColours(surface, ramp) {
    var range = heightRange(surface), span = range.max - range.min, colours = [];
    for (var i = 0; i < surface.heightsMm.length; i++) {
      var t = span > 0 ? (surface.heightsMm[i] - range.min) / span : 0.5;
      var colour = rampColour(ramp, t);
      colours.push(colour[0], colour[1], colour[2]);
    }
    return colours;
  }
  function currentSurface() { return model.surfaces[state.surface]; }

  function render() {
    if (!model || !gl) { return; }
    // G29: hiding the layer of the selected object clears the selection instead of
    // leaving a card and a highlight for an object the user can no longer see.
    if (state.selection && selectionLayerHidden(state.selection)) { clearSelection(); return; }
    var started = performance.now();
    var canvas = resizeCanvas();
    gl.viewport(0, 0, canvas.width, canvas.height);
    gl.clearColor(BACKGROUND[0], BACKGROUND[1], BACKGROUND[2], 1);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    var camera = state.camera, eye = cameraEye();
    var limits = cameraLimits();
    camera.minDistance = limits.min; camera.maxDistance = limits.max;
    var mvp = mat4Multiply(perspective(0.9, canvas.width / canvas.height, 0.5, 2000), lookAt(eye, camera.target, [0, 1, 0]));
    gl.uniformMatrix4fv(uniforms.mvp, false, mvp);
    gl.uniform3fv(uniforms.eye, new Float32Array(eye));
    gl.uniform2fv(uniforms.fogRange, new Float32Array([limits.span * FOG_NEAR_FACTOR, limits.span * FOG_FAR_FACTOR]));
    var surface = currentSurface();
    var material = MATERIALS[state.surface];
    state.surfaceSource = surface.source;
    state.surfaceMaterial = material.label;
    state.vertexCount = surface.vertexCount;
    state.triangleCount = surface.triangles;
    state.surfaceVertexCount = surface.vertexCount;
    state.modelDeclaredTerrainTriangles = surface.triangles;
    state.modelDeclaredTerrainVertices = model.grid[state.surface] ? model.grid[state.surface].vertexCount : null;
    state.drawCalls = 0;
    state.indexedDrawCalls = 0;
    state.solidDrawCalls = 0;
    state.submittedIndices = 0;
    state.entitySolidsDrawn = 0;
    state.roadSolidsDrawn = 0;
    state.drawnTriangleCount = 0;
    state.entityRoadTriangleCount = 0;
    // Real terrain topology: validated once per surface, cached, then submitted
    // through the indexed path only. An invalid topology throws and is reported
    // instead of silently falling back to a wrong non-indexed draw.
    var indices = terrainIndicesFor(state.surface);
    state.terrainIndexCount = indices.length;
    state.terrainTriangleCount = indices.length / 3;
    state.terrainIndicesBySurface[state.surface] = indices.length;
    state.terrainTrianglesBySurface[state.surface] = indices.length / 3;
    state.terrainTopologyValidated = true;
    drawIndexedTriangles(surface.verticesMetres, surfaceColours(surface, material.ramp), indices, "terrain " + state.surface);
    state.terrainDrawnWithIndexBuffer = true;
    state.surfaceTriangleCount = indices.length / 3;
    state.drawnTriangleCount += state.surfaceTriangleCount;
    if (document.getElementById("showRoads").checked) {
      for (var r = 0; r < model.roads.length; r++) {
        var road = model.roads[r];
        if (road.runsNo3dPrimitive) { continue; }
        var roadMesh = flattenTriangles(solidTriangles(road), road.roadId === selectedId ? MATERIALS.roadSelected : MATERIALS.road);
        if (roadMesh.vertices.length) { drawSolidTriangles(roadMesh.vertices, roadMesh.colours); state.roadSolidsDrawn++; state.entityRoadTriangleCount += roadMesh.vertices.length / 9; state.drawnTriangleCount += roadMesh.vertices.length / 9; }
      }
    }
    if (document.getElementById("showEntities").checked) {
      for (var e = 0; e < model.entities.length; e++) {
        var entity = model.entities[e];
        if (entity.runsNo3dPrimitive) { continue; }
        var mesh = flattenTriangles(solidTriangles(entity), entity.entityId === selectedId ? MATERIALS.entitySelected : MATERIALS.entity);
        if (mesh.vertices.length) { drawSolidTriangles(mesh.vertices, mesh.colours); state.entitySolidsDrawn++; state.entityRoadTriangleCount += mesh.vertices.length / 9; state.drawnTriangleCount += mesh.vertices.length / 9; }
      }
    }
    state.visibleLayers = ["terrain:" + state.surface];
    state.hiddenLayers = [];
    if (document.getElementById("showEntities").checked) { state.visibleLayers.push("entities"); } else { state.hiddenLayers.push("entities"); }
    if (document.getElementById("showRoads").checked) { state.visibleLayers.push("roads"); } else { state.hiddenLayers.push("roads"); }
    if (state.overlayVisible) { state.visibleLayers.push("observation2d"); } else { state.hiddenLayers.push("observation2d"); }
    state.renderedPixelVariety = pixelVariety(canvas);
    drawObservationOverlay();
    drawMinimap();
    state.renderCount++;
    state.frameMs = round6(performance.now() - started);
    // G29: the measured map rectangle is published on every render, so a mode
    // switch that moves the facts panel is visible in the page state too.
    measureLayout();
    // G28-R1-H1: the HUD is refreshed after every render, so the text the user
    // sees (layer source, draw path, drawn triangles, overlay state) can never be
    // a stale boot snapshot. Saved-fact counts always come from model.status and
    // are never replaced by derived numbers. This holds in both display modes:
    // browse mode hides the facts panel, it never stops updating it.
    refreshHud();
  }
  function pixelVariety(canvas) {
    var pixels = new Uint8Array(canvas.width * canvas.height * 4);
    gl.readPixels(0, 0, canvas.width, canvas.height, gl.RGBA, gl.UNSIGNED_BYTE, pixels);
    var seen = {}, count = 0;
    var step = Math.max(1, Math.floor(canvas.width * canvas.height / 400)) * 4;
    for (var i = 0; i < pixels.length; i += step) {
      var key = pixels[i] + "," + pixels[i + 1] + "," + pixels[i + 2];
      if (!seen[key]) { seen[key] = true; count++; }
    }
    return count;
  }

  // --------------------------------------------------------------- 2D overlay
  function drawObservationOverlay() {
    var canvas = document.getElementById("overlay");
    if (!canvas) { return; }
    var context = canvas.getContext("2d");
    if (!context) { return; }
    context.clearRect(0, 0, canvas.width, canvas.height);
    state.overlayRecordsDrawn = 0;
    if (!model || !state.overlayVisible) { return; }
    var win = model.window;
    var pad = 16, width = canvas.width - pad * 2, height = canvas.height - pad * 2;
    if (width < 40 || height < 40) { return; }
    var spanX = Math.max(win.widthM, 1), spanZ = Math.max(win.depthM, 1);
    var toX = function (x) { return pad + ((x - win.minX) / spanX) * width; };
    var toY = function (z) { return canvas.height - pad - ((z - win.minZ) / spanZ) * height; };
    context.save();
    context.fillStyle = "rgba(10,13,18,0.55)";
    context.fillRect(pad - 6, pad - 6, width + 12, height + 12);
    context.strokeStyle = "rgba(150,170,200,0.35)";
    context.lineWidth = 1;
    context.strokeRect(pad - 6, pad - 6, width + 12, height + 12);
    // access observation polylines: saved x/z only, never a world height
    for (var i = 0; i < model.access.length; i++) {
      var record = model.access[i];
      var path = record.path || [];
      if (path.length < 2) { continue; }
      context.setLineDash([5, 4]);
      context.strokeStyle = record.inWindow ? OBSERVATION_COLOURS.access : OBSERVATION_COLOURS.accessFar;
      context.beginPath();
      for (var j = 0; j < path.length; j++) {
        var px = toX(path[j].x), py = toY(path[j].z);
        if (j === 0) { context.moveTo(px, py); } else { context.lineTo(px, py); }
      }
      context.stroke();
      state.overlayRecordsDrawn++;
    }
    context.setLineDash([]);
    // node records: saved x/z marks
    for (var n = 0; n < model.nodes.length; n++) {
      var node = model.nodes[n];
      if (typeof node.worldX !== "number" || typeof node.worldZ !== "number") { continue; }
      var nx = toX(node.worldX), ny = toY(node.worldZ);
      context.fillStyle = OBSERVATION_COLOURS.nodes;
      context.fillRect(nx - 3, ny - 3, 6, 6);
      state.overlayRecordsDrawn++;
    }
    context.fillStyle = "rgba(226,232,240,0.92)";
    context.font = "11px ui-monospace,Consolas,monospace";
    context.fillText(OBSERVATION_LABEL, pad, canvas.height - 4);
    context.fillText("window x " + win.minX + ".." + (win.minX + win.widthM) + "  z " + win.minZ + ".." + (win.minZ + win.depthM) + " (plan)", pad, pad - 10 < 12 ? 12 : pad - 10);
    context.restore();
  }

  // ----------------------------------------------------------------- picking
  function vertexAt(vertices, index) {
    var base = index * 3;
    if (base < 0 || base + 2 >= vertices.length) { return null; }
    var point = [vertices[base], vertices[base + 1], vertices[base + 2]];
    return point.every(isFinite) ? point : null;
  }
  function trianglesOf(corners, indices) {
    var out = [];
    if (!corners || !indices || indices.length % 3 !== 0) { return out; }
    for (var t = 0; t + 2 < indices.length; t += 3) {
      var a = corners[indices[t]], b = corners[indices[t + 1]], c = corners[indices[t + 2]];
      if (!a || !b || !c) { continue; }
      out.push([a, b, c]);
    }
    return out;
  }
  function solidTriangles(entry) { return trianglesOf(entry.cornersMetres, entry.triangleIndices); }
  // The pick set is built from the very same validated index grid the draw call
  // submits, so a hit can only ever be on a drawn terrain triangle.
  function surfacePickData(name) {
    var surface = model.surfaces[name];
    if (!surface) { throw new Error("the model declares no surface " + name); }
    var indices = terrainIndicesFor(name);
    var vertices = surface.verticesMetres;
    var triangles = [], sourceIndices = [], triangleNumbers = [];
    for (var i = 0; i + 2 < indices.length; i += 3) {
      var a = vertexAt(vertices, indices[i]);
      var b = vertexAt(vertices, indices[i + 1]);
      var c = vertexAt(vertices, indices[i + 2]);
      if (!a || !b || !c) { continue; }
      triangles.push([a, b, c]);
      sourceIndices.push([indices[i], indices[i + 1], indices[i + 2]]);
      triangleNumbers.push(i / 3);
    }
    return { triangles: triangles, sourceIndices: sourceIndices, triangleNumbers: triangleNumbers };
  }
  function rayTriangle(origin, dir, a, b, c) {
    var edge1 = sub3(b, a), edge2 = sub3(c, a);
    var pvec = cross3(dir, edge2), det = dot3(edge1, pvec);
    if (Math.abs(det) < 1e-12) { return null; }
    var inv = 1 / det, tvec = sub3(origin, a), u = dot3(tvec, pvec) * inv;
    if (u < -1e-9 || u > 1 + 1e-9) { return null; }
    var qvec = cross3(tvec, edge1), v = dot3(dir, qvec) * inv;
    if (v < -1e-9 || u + v > 1 + 1e-9) { return null; }
    var t = dot3(edge2, qvec) * inv;
    return t > 1e-9 ? t : null;
  }
  function barycentricOf(point, a, b, c) {
    var normal = cross3(sub3(b, a), sub3(c, a)), area2 = dot3(normal, normal);
    if (area2 <= 0) { return null; }
    var wa = dot3(cross3(sub3(b, point), sub3(c, point)), normal) / area2;
    var wb = dot3(cross3(sub3(c, point), sub3(a, point)), normal) / area2;
    return [wa, wb, 1 - wa - wb];
  }
  function nearestIn(triangles, ray, numbers) {
    var best = null;
    for (var i = 0; i < triangles.length; i++) {
      var t = rayTriangle(ray.origin, ray.dir, triangles[i][0], triangles[i][1], triangles[i][2]);
      if (t === null) { continue; }
      if (!best || t < best.t) {
        var point = [ray.origin[0] + ray.dir[0] * t, ray.origin[1] + ray.dir[1] * t, ray.origin[2] + ray.dir[2] * t];
        best = {
          t: t, point: point, slot: i,
          triangleIndex: numbers ? numbers[i] : i,
          barycentric: barycentricOf(point, triangles[i][0], triangles[i][1], triangles[i][2])
        };
      }
    }
    return best;
  }
  function flattenTriangles(triangles, colour) {
    var vertices = [], colours = [];
    for (var i = 0; i < triangles.length; i++) {
      for (var corner = 0; corner < 3; corner++) {
        vertices.push(triangles[i][corner][0], triangles[i][corner][1], triangles[i][corner][2]);
        colours.push(colour[0], colour[1], colour[2]);
      }
    }
    return { vertices: vertices, colours: colours };
  }
  function rayFromClick(px, py) {
    var canvas = document.getElementById("gl");
    var eye = cameraEye();
    var mvp = mat4Multiply(perspective(0.9, canvas.width / canvas.height, 0.5, 2000), lookAt(eye, state.camera.target, [0, 1, 0]));
    var inv = invert(mvp);
    if (!inv) { return null; }
    var ndcX = (px / canvas.width) * 2 - 1, ndcY = 1 - (py / canvas.height) * 2;
    function unproject(z) {
      var v = [ndcX, ndcY, z, 1], out = [0, 0, 0, 0];
      for (var i = 0; i < 4; i++) { out[i] = inv[0 * 4 + i] * v[0] + inv[1 * 4 + i] * v[1] + inv[2 * 4 + i] * v[2] + inv[3 * 4 + i] * v[3]; }
      return [out[0] / out[3], out[1] / out[3], out[2] / out[3]];
    }
    var near = unproject(-1), far = unproject(1);
    var dir = [far[0] - near[0], far[1] - near[1], far[2] - near[2]];
    var len = Math.hypot(dir[0], dir[1], dir[2]) || 1;
    return { origin: near, dir: [dir[0] / len, dir[1] / len, dir[2] / len] };
  }
  function invert(m) {
    var inv = new Float64Array(16), det;
    inv[0] = m[5] * m[10] * m[15] - m[5] * m[11] * m[14] - m[9] * m[6] * m[15] + m[9] * m[7] * m[14] + m[13] * m[6] * m[11] - m[13] * m[7] * m[10];
    inv[4] = -m[4] * m[10] * m[15] + m[4] * m[11] * m[14] + m[8] * m[6] * m[15] - m[8] * m[7] * m[14] - m[12] * m[6] * m[11] + m[12] * m[7] * m[10];
    inv[8] = m[4] * m[9] * m[15] - m[4] * m[11] * m[13] - m[8] * m[5] * m[15] + m[8] * m[7] * m[13] + m[12] * m[5] * m[11] - m[12] * m[7] * m[9];
    inv[12] = -m[4] * m[9] * m[14] + m[4] * m[10] * m[13] + m[8] * m[5] * m[14] - m[8] * m[6] * m[13] - m[12] * m[5] * m[10] + m[12] * m[6] * m[9];
    inv[1] = -m[1] * m[10] * m[15] + m[1] * m[11] * m[14] + m[9] * m[2] * m[15] - m[9] * m[3] * m[14] - m[13] * m[2] * m[11] + m[13] * m[3] * m[10];
    inv[5] = m[0] * m[10] * m[15] - m[0] * m[11] * m[14] - m[8] * m[2] * m[15] + m[8] * m[3] * m[14] + m[12] * m[2] * m[11] - m[12] * m[3] * m[10];
    inv[9] = -m[0] * m[9] * m[15] + m[0] * m[11] * m[13] + m[8] * m[1] * m[15] - m[8] * m[3] * m[13] - m[12] * m[1] * m[11] + m[12] * m[3] * m[9];
    inv[13] = m[0] * m[9] * m[14] - m[0] * m[10] * m[13] - m[8] * m[1] * m[14] + m[8] * m[2] * m[13] + m[12] * m[1] * m[10] - m[12] * m[2] * m[9];
    inv[2] = m[1] * m[6] * m[15] - m[1] * m[7] * m[14] - m[5] * m[2] * m[15] + m[5] * m[3] * m[14] + m[13] * m[2] * m[7] - m[13] * m[3] * m[6];
    inv[6] = -m[0] * m[6] * m[15] + m[0] * m[7] * m[14] + m[4] * m[2] * m[15] - m[4] * m[3] * m[14] - m[12] * m[2] * m[7] + m[12] * m[3] * m[6];
    inv[10] = m[0] * m[5] * m[15] - m[0] * m[7] * m[13] - m[4] * m[1] * m[15] + m[4] * m[3] * m[13] + m[12] * m[1] * m[7] - m[12] * m[3] * m[5];
    inv[14] = -m[0] * m[5] * m[14] + m[0] * m[6] * m[13] + m[4] * m[1] * m[14] - m[4] * m[2] * m[13] - m[12] * m[1] * m[6] + m[12] * m[2] * m[5];
    inv[3] = -m[1] * m[6] * m[11] + m[1] * m[7] * m[10] + m[5] * m[2] * m[11] - m[5] * m[3] * m[10] - m[9] * m[2] * m[7] + m[9] * m[3] * m[6];
    inv[7] = m[0] * m[6] * m[11] - m[0] * m[7] * m[10] - m[4] * m[2] * m[11] + m[4] * m[3] * m[10] + m[8] * m[2] * m[7] - m[8] * m[3] * m[6];
    inv[11] = -m[0] * m[5] * m[11] + m[0] * m[7] * m[9] + m[4] * m[1] * m[11] - m[4] * m[3] * m[9] - m[8] * m[1] * m[7] + m[8] * m[3] * m[5];
    inv[15] = m[0] * m[5] * m[10] - m[0] * m[6] * m[9] - m[4] * m[1] * m[10] + m[4] * m[2] * m[9] + m[8] * m[1] * m[6] - m[8] * m[2] * m[5];
    det = m[0] * inv[0] + m[1] * inv[4] + m[2] * inv[8] + m[3] * inv[12];
    if (!det) { return null; }
    for (var i = 0; i < 16; i++) { inv[i] = inv[i] / det; }
    return inv;
  }

  function clearSelection() {
    state.selection = null;
    state.selectedEntityId = null;
    state.selectedFacts = null;
    state.terrainHit = null;
    state.roadHit = null;
    state.pickObject = null;
    selectedId = null;
    renderSelection();
    render();
  }

  /// G29: which layer checkbox governs a selection. The terrain is always drawn,
  /// so it never makes a selection stale.
  function selectionLayerHidden(selection) {
    if (!selection) { return false; }
    if (selection.kind === "entity") { return !document.getElementById("showEntities").checked; }
    if (selection.kind === "road") { return !document.getElementById("showRoads").checked; }
    return false;
  }

  function pick(px, py) {
    var ray = rayFromClick(px, py);
    state.pickObject = null;
    if (!ray) { return null; }
    state.pickMethod = "page.mouse.click";
    state.pickCount++;
    // Priority follows the drawn layers: entity solid, then road bed, then the
    // drawn terrain triangles. A hidden layer contributes no triangle.
    var bestEntity = null, bestEntityHit = null;
    if (document.getElementById("showEntities").checked) {
      for (var i = 0; i < model.entities.length; i++) {
        var entity = model.entities[i];
        if (entity.runsNo3dPrimitive) { continue; }
        var hit = nearestIn(solidTriangles(entity), ray, null);
        if (hit && (!bestEntityHit || hit.t < bestEntityHit.t)) { bestEntityHit = hit; bestEntity = entity; }
      }
    }
    if (bestEntity && bestEntityHit) {
      selectedId = bestEntity.entityId;
      state.selectedEntityId = bestEntity.entityId;
      state.pickObject = "entitySolid";
      state.selectedFacts = {
        entityId: bestEntity.entityId, kind: bestEntity.kind, assetId: bestEntity.assetId,
        worldX: bestEntity.worldX, worldY: bestEntity.worldY, worldZ: bestEntity.worldZ,
        rotationYDeg: bestEntity.rotationYDeg, scale: bestEntity.scale,
        widthM: bestEntity.widthM, depthM: bestEntity.depthM, heightM: bestEntity.heightM,
        savedBounds: bestEntity.savedBounds, heightFactMm: bestEntity.heightFactMm,
        representation: bestEntity.representation,
        triangleIndex: bestEntityHit.triangleIndex, barycentric: bestEntityHit.barycentric,
        derivedHitMetres: { x: bestEntityHit.point[0], y: bestEntityHit.point[1], z: bestEntityHit.point[2] }
      };
      state.selection = { kind: "entity", label: "entity solid", facts: state.selectedFacts, objectKey: "entity:" + bestEntity.entityId };
      state.terrainHit = null; state.roadHit = null;
      state.selectionIsSameObjectAsHighlight = true;
      renderSelection(); render();
      return state.selectedFacts;
    }
    var bestRoad = null, bestRoadHit = null;
    if (document.getElementById("showRoads").checked) {
      for (var r = 0; r < model.roads.length; r++) {
        var road = model.roads[r];
        if (road.runsNo3dPrimitive) { continue; }
        var roadHit = nearestIn(solidTriangles(road), ray, null);
        if (roadHit && (!bestRoadHit || roadHit.t < bestRoadHit.t)) { bestRoadHit = roadHit; bestRoad = road; }
      }
    }
    if (bestRoad && bestRoadHit) {
      selectedId = bestRoad.roadId;
      state.pickObject = "roadSolid";
      state.roadHit = {
        roadId: bestRoad.roadId, representation: bestRoad.representation,
        worldX: bestRoad.worldX, worldZ: bestRoad.worldZ,
        lengthM: bestRoad.lengthM, widthM: bestRoad.widthM, rotationYDeg: bestRoad.rotationYDeg,
        heightFactMm: bestRoad.heightFactMm, savedBounds: bestRoad.savedBounds,
        triangleIndex: bestRoadHit.triangleIndex, barycentric: bestRoadHit.barycentric,
        derivedHitMetres: { x: bestRoadHit.point[0], y: bestRoadHit.point[1], z: bestRoadHit.point[2] }
      };
      state.selection = { kind: "road", label: "road bed solid", facts: state.roadHit, objectKey: "road:" + bestRoad.roadId };
      state.selectedEntityId = null; state.selectedFacts = null; state.terrainHit = null;
      state.selectionIsSameObjectAsHighlight = true;
      renderSelection(); render();
      return state.roadHit;
    }
    var surface = currentSurface();
    var data = surfacePickData(state.surface);
    var terrainHit = nearestIn(data.triangles, ray, data.triangleNumbers);
    if (!terrainHit) {
      state.pickMisses++;
      clearSelection();
      return null;
    }
    var width = model.grid[state.surface].sampleWidth;
    var source = data.sourceIndices[terrainHit.slot];
    var cell = Math.floor(terrainHit.triangleIndex / 2);
    var columns = width - 1;
    selectedId = null;
    state.pickObject = "terrainTriangle";
    state.selectedEntityId = null;
    state.selectedFacts = null;
    state.roadHit = null;
    state.terrainHit = {
      surface: state.surface, source: surface.source, triangleIndex: terrainHit.triangleIndex, cellIndex: cell,
      cellX: cell % columns, cellZ: Math.floor(cell / columns),
      sourceSampleIndices: source,
      sourceSampleHeightsMm: [surface.heightsMm[source[0]], surface.heightsMm[source[1]], surface.heightsMm[source[2]]],
      barycentric: terrainHit.barycentric,
      derivedHitMetres: { x: terrainHit.point[0], y: terrainHit.point[1], z: terrainHit.point[2] },
      derivedHitHeightMm: Math.round(terrainHit.point[1] * 1000),
      interpolation: "barycentricOnDrawnTriangles",
      domain: "derivedDisplayGeometry", notSavedMillimetreFact: true
    };
    state.selection = { kind: "terrain", label: "terrain triangle", facts: { surface: state.surface, source: surface.source, triangleIndex: state.terrainHit.triangleIndex, cellIndex: cell, sourceSampleIndices: source, sourceSampleHeightsMm: state.terrainHit.sourceSampleHeightsMm }, derived: state.terrainHit, objectKey: "terrain:" + state.surface + ":" + state.terrainHit.triangleIndex };
    state.selectionIsSameObjectAsHighlight = null;
    renderSelection(); render();
    return state.terrainHit;
  }

  function selectionRows() {
    var selection = state.selection;
    if (!selection) { return null; }
    var rows = [], derived = [];
    var facts = selection.facts || {};
    rows.push(["object", selection.label]);
    rows.push(["objectKey", selection.objectKey]);
    if (selection.kind === "entity" || selection.kind === "road") {
      rows.push(["entityId / roadId", selection.kind === "entity" ? facts.entityId : facts.roadId]);
      if (selection.kind === "entity") { rows.push(["kind", facts.kind]); rows.push(["assetId", facts.assetId]); }
      rows.push(["worldXYZ (saved)", facts.worldX + ", " + (facts.worldY === undefined ? "(no saved worldY)" : facts.worldY) + ", " + facts.worldZ]);
      rows.push(["rotationYDeg (saved)", facts.rotationYDeg]);
      if (selection.kind === "entity") { rows.push(["scale (saved)", facts.scale]); }
      rows.push(["widthM / depthM / heightM (saved)", facts.widthM + " / " + facts.depthM + " / " + facts.heightM]);
      rows.push(["savedBounds (saved world AABB)", JSON.stringify(facts.savedBounds)]);
      rows.push(["heightFactMm (saved)", facts.heightFactMm === undefined ? "(none saved)" : facts.heightFactMm]);
      rows.push(["representation", facts.representation]);
    } else {
      rows.push(["surface", facts.surface]);
      rows.push(["surface source", facts.source]);
      rows.push(["triangleIndex", facts.triangleIndex]);
      rows.push(["cellIndex", facts.cellIndex]);
      rows.push(["source sample indices", (facts.sourceSampleIndices || []).join(", ")]);
      rows.push(["source sample heights (saved mm)", (facts.sourceSampleHeightsMm || []).join(", ")]);
    }
    if (selection.derived) {
      derived.push(["derived hit (display geometry)", selection.derived.derivedHitMetres.x.toFixed(4) + ", " + selection.derived.derivedHitMetres.y.toFixed(4) + ", " + selection.derived.derivedHitMetres.z.toFixed(4)]);
      derived.push(["derived hit height (mm)", selection.derived.derivedHitHeightMm]);
      derived.push(["interpolation", selection.derived.interpolation]);
    } else {
      derived.push(["derived hit triangle (display geometry)", facts.triangleIndex]);
      derived.push(["derived hit barycentric", (facts.barycentric || []).map(function (w) { return w.toFixed(4); }).join(", ")]);
      derived.push(["derived hit metres (display geometry)", facts.derivedHitMetres.x.toFixed(4) + ", " + facts.derivedHitMetres.y.toFixed(4) + ", " + facts.derivedHitMetres.z.toFixed(4)]);
    }
    return { rows: rows, derived: derived };
  }

  function cardField(label, value) {
    return '<span><span class="k">' + escapeHtml(label) + '</span> <span class="v">' + escapeHtml(value) + '</span></span>';
  }
  /// G29: the concise card that browse mode shows. It names the object and its
  /// key saved dimensions or coordinates only; the complete saved field set and
  /// the derived pick intersection stay behind one control. A terrain selection
  /// keeps its explicit "derived, never a saved sample" note.
  function selectionSummaryHtml() {
    var selection = state.selection || {};
    var facts = selection.facts || {};
    var parts = [];
    if (selection.kind === "entity" || selection.kind === "road") {
      parts.push(cardField("对象", selection.label));
      parts.push(cardField(selection.kind === "entity" ? "entityId" : "roadId", selection.kind === "entity" ? facts.entityId : facts.roadId));
      if (selection.kind === "entity") { parts.push(cardField("kind", facts.kind)); }
      parts.push(cardField("worldXYZ (saved)", facts.worldX + " / " + (facts.worldY === undefined ? "(no saved worldY)" : facts.worldY) + " / " + facts.worldZ));
      parts.push(cardField("宽/深/高 m (saved)", facts.widthM + " / " + facts.depthM + " / " + facts.heightM));
    } else {
      parts.push(cardField("对象", selection.label));
      parts.push(cardField("surface", facts.surface + " (" + facts.source + ")"));
      parts.push(cardField("triangleIndex", facts.triangleIndex));
      parts.push(cardField("派生命中高度 mm", selection.derived ? selection.derived.derivedHitHeightMm : "-"));
      parts.push('<span class="note">派生显示值：非保存样本、不写回模型</span>');
    }
    return '<div class="card">' + parts.join("") + '</div>';
  }

  function renderSelection() {
    var host = document.getElementById("selection");
    if (!host) { return; }
    var rows = selectionRows();
    if (!rows) {
      host.innerHTML = '<div class="muted">nothing selected - click an entity solid, a road bed or the terrain (click the background to clear)</div>';
      state.selectionPanelText = "nothing selected";
      return;
    }
    var html = selectionSummaryHtml();
    // One control expands the complete saved facts and the derived intersection.
    // The open flag lives on the state, so a mode switch cannot accumulate a
    // second copy or leak the previous mode's layout.
    html += '<details id="selection-details"' + (state.selectionDetailsOpen ? " open" : "") + '><summary>完整保存字段与派生交点（当前显示模式：' + (state.viewMode === "inspect" ? "检查" : "浏览") + '）</summary>';
    html += '<table class="sel"><tbody>';
    for (var i = 0; i < rows.rows.length; i++) {
      html += '<tr><th>' + escapeHtml(rows.rows[i][0]) + '</th><td class="fact">' + escapeHtml(rows.rows[i][1]) + '</td></tr>';
    }
    for (var j = 0; j < rows.derived.length; j++) {
      html += '<tr><th>' + escapeHtml(rows.derived[j][0]) + '</th><td class="derived">' + escapeHtml(rows.derived[j][1]) + '</td></tr>';
    }
    html += '</tbody></table><div class="legend"><span class="fact">saved fact</span> <span class="derived">derived display value (never written back as a saved millimetre fact)</span></div></details>';
    host.innerHTML = html;
    state.selectionPanelText = rows.rows.map(function (row) { return row[0] + ": " + row[1]; }).join(" | ");
  }

  /// Saved-fact counts. Every number here comes from `model.status`; derived
  /// render numbers are never substituted into this block. Only the overlay
  /// shown/hidden suffix is dynamic.
  function hudCountsHtml(status) {
    var byCategory = status.categoryCounts || {};
    var entities = byCategory.entities || {}, roads = byCategory.roads || {};
    var nodes = byCategory.nodes || {}, access = byCategory.access || {};
    var line = function (label, entry, total) {
      var solid = (entry.solid === undefined) ? "" : " (" + entry.solid + " drawable solids)";
      return "<div>" + label + ": <span class=\"ok\">" + total + "</span>" + solid + "</div>";
    };
    return line("entities", entities, entities.total) +
      line("roads", roads, roads.total) +
      "<div>nodes: <span class=\"ok\">" + nodes.total + "</span> records (observation only)</div>" +
      "<div>access: <span class=\"ok\">" + access.total + "</span> observation records</div>" +
      "<div>observation2d: <span class=\"warn\">" + (status.observation2d ? status.observation2d.total : 0) + "</span> total = " + (status.observation2d ? status.observation2d.nodes : 0) + " nodes + " + (status.observation2d ? status.observation2d.access : 0) + " access" +
      (state.overlayVisible ? " (2D plan overlay shown)" : " (2D plan overlay hidden)") + "</div>" +
      "<div>no saved height: entities " + entities.noHeight + "/" + entities.total + ", roads " + roads.noHeight + "/" + roads.total + ", nodes " + nodes.noHeight + "/" + nodes.total + ", access " + access.noHeight + "/" + access.total + " (" + status.heightlessObjectCount + " objects)</div>" +
      "<div>not positioned in 3D: entities " + entities.notPositioned + "/" + entities.total + ", roads " + roads.notPositioned + "/" + roads.total + ", nodes " + nodes.notPositioned + "/" + nodes.total + ", access " + access.notPositioned + "/" + access.total + " (" + status.notPositionedCount + " objects)</div>" +
      "<div>not snapped to a surface: entities " + entities.notSnapped + "/" + entities.total + ", roads " + roads.notSnapped + "/" + roads.total + ", nodes " + nodes.notSnapped + "/" + nodes.total + ", access " + access.notSnapped + "/" + access.total + " (snapped: " + status.snappedCount + "; snapping: " + status.snapping + ")</div>" +
      "<div class=\"" + (status.constantWorldYUsed ? "err" : "ok") + "\">constant world Y used: " + status.constantWorldYUsed + "</div>";
  }
  function hudGlHtml() {
    return "<div>WebGL2: <span class=\"" + (state.isWebGL2 ? "ok" : "err") + "\">" + state.isWebGL2 + "</span></div>" +
      "<div>version: " + escapeHtml(state.glVersion) + "</div>" +
      "<div>renderer: " + escapeHtml(state.glRenderer) + "</div>" +
      "<div>load: " + (state.loadMs === null ? "n/a" : state.loadMs + " ms") + " | last frame: " + (state.frameMs === null ? "n/a" : state.frameMs + " ms") + " | draw calls: " + state.drawCalls + "</div>";
  }
  function hudLayerHtml(status) {
    return "<div>layer: <span class=\"ok\">" + escapeHtml(state.surfaceMaterial) + "</span> from " + escapeHtml(state.surfaceSource) + "</div>" +
      "<div>terrain mesh: vertices " + state.surfaceVertexCount + " | model triangles " + state.modelDeclaredTerrainTriangles + " | <span class=\"ok\">submitted indices " + state.terrainIndexCount + "</span> | terrain triangles " + state.terrainTriangleCount + "</div>" +
      "<div>draw path: " + (state.terrainDrawnWithIndexBuffer ? "indexed terrain (drawElements)" : "TERRAIN NOT INDEXED") + " | indexed draws " + state.indexedDrawCalls + " | solid draws " + state.solidDrawCalls + "</div>" +
      "<div>drawn triangles: " + state.drawnTriangleCount + " (terrain " + state.terrainTriangleCount + " + entity/road solids " + state.entityRoadTriangleCount + ")</div>" +
      "<div>layers: " + (state.visibleLayers || []).join(", ") + (state.hiddenLayers && state.hiddenLayers.length ? " | hidden: " + state.hiddenLayers.join(", ") : "") + "</div>" +
      "<div>2D observation overlay: " + (state.overlayVisible ? "shown" : "hidden") + " | records drawn " + state.overlayRecordsDrawn + "</div>" +
      "<div>natural == final: " + status.naturalEqualsFinal + "</div>";
  }
  function hudEvidenceHtml(status) {
    return "<div>scene: " + escapeHtml(model.sources.sceneId || "none") + "</div>" +
      "<div>window: " + model.window.minX + "," + model.window.minZ + "," + model.window.widthM + "," + model.window.depthM + "</div>" +
      "<div class=\"" + (status.schemaDriftPresent ? "warn" : "ok") + "\">schema drift: " + escapeHtml(model.schemaDrift || "none") + "</div>" +
      "<div class=\"" + (status.unmodelledKindCount > 0 ? "warn" : "ok") + "\">source fact / not converted kinds: " + status.unmodelledKindCount + "</div>" +
      "<div><span class=\"badge\">sourceObservationOnly</span><span class=\"badge\">notReverified</span> access records are observations, never verified routes</div>" +
      "<div>background: " + BACKGROUND.join(", ") + " | fog: " + FOG_COLOUR.join(", ") + " | " + escapeHtml(state.lighting.model) + "</div>";
  }
  function hudText(id) { var element = document.getElementById(id); return element ? (element.textContent || "") : ""; }

  /// G28-R1-H1: refreshes every HUD line whose value can change between renders
  /// (layer and source, mesh and draw path, drawn triangles, layer and overlay
  /// state) and publishes the same strings on `state.hud`, so the visible text,
  /// `__PREVIEW3D_STATE__` and `__terrainRuntime3d()` can be compared item by
  /// item instead of trusting a boot snapshot.
  // ------------------------------------------------------- G30 reading aids
  /// The view basis of the live camera, taken from the same lookAt() the render
  /// path uses, so the direction reading follows rotation instead of a fixed
  /// screen-up assumption.
  function viewBasis() {
    var m = lookAt(cameraEye(), state.camera.target || [0, 0, 0], [0, 1, 0]);
    // lookAt() stores column-major, so the rotation rows are the basis vectors:
    // row r lives at m[r], m[4 + r], m[8 + r]. Reading the columns here would
    // transpose the basis and flip the projected Z axis (found by the G30
    // independent view-basis check).
    return { x: [m[0], m[4], m[8]], y: [m[1], m[5], m[9]], z: [m[2], m[6], m[10]] };
  }
  /// Screen projection of every local world axis. A projection that collapses is
  /// reported as degenerate with a reason instead of a fabricated direction.
  function directionReading() {
    if (!model) { return null; }
    var basis = viewBasis();
    var axes = {}, degenerate = [], degenerateShort = [];
    for (var i = 0; i < WORLDAXES.length; i++) {
      var axis = WORLDAXES[i];
      var screenX = dot3(axis.vector, basis.x);
      var screenY = -dot3(axis.vector, basis.y);
      var length = Math.hypot(screenX, screenY);
      var entry = { key: axis.key, label: axis.label, screenX: round6(screenX), screenY: round6(screenY), screenLength: round6(length), degenerate: length < AXIS_DEGENERATE_LENGTH };
      if (entry.degenerate) { entry.reason = "the view direction is nearly parallel to this axis, so the screen cannot show it"; degenerate.push(axis.label); degenerateShort.push(axis.short); }
      axes[axis.key] = entry;
    }
    return {
      axes: axes,
      frameStatement: "本地世界坐标：X 东、Y 上、Z 南（局部坐标轴，不是测绘真北）",
      camera: { yaw: round6(state.camera.yaw), pitch: round6(state.camera.pitch), mode: state.camera.mode },
      degenerateAxes: degenerate,
      degenerateShort: degenerateShort,
      degenerate: degenerate.length > 0
    };
  }
  /// Legend entries. Every swatch is read from the render configuration, so the
  /// legend cannot invent a colour that the map does not use.
  function legendReading() {
    var material = MATERIALS[state.surface];
    var hidden = state.hiddenLayers || [];
    var entries = [
      { id: "terrain", label: "地形 " + state.surface + "（" + material.source + "）", summaryLabel: "地形 " + state.surface + (state.surface === "final" ? " 最终" : " 自然"), swatch: rgbCss(rampColour(material.ramp, 0.5)), hidden: false, note: "基础材质色；实际像素还受光照与雾影响" },
      { id: "terrain-ramp", kind: "ramp", label: "保存样本色带", from: rgbCss(rampColour(material.ramp, 0)), to: rgbCss(rampColour(material.ramp, 1)), hidden: false, note: "同一 rampColour() 按保存样本高度映射，未改数值" },
      { id: "entities", label: "实体（保存朝向的块体）", summaryLabel: "实体", swatch: rgbCss(MATERIALS.entity), hidden: hidden.indexOf("entities") >= 0, note: "按保存的 worldX/worldZ 与朝向放置的块体" },
      { id: "roads", label: "道路（保存朝向的路床）", summaryLabel: "道路", swatch: rgbCss(MATERIALS.road), hidden: hidden.indexOf("roads") >= 0, note: "按保存的走向与宽度铺设的路床" },
      { id: "observation2d", label: "2D 观测：仅保存 x/z，未贴高、未复核路线", summaryLabel: "2D观测（无世界Y·未复核）", swatch: OBSERVATION_COLOURS.nodes, hidden: hidden.indexOf("observation2d") >= 0, extraSwatch: OBSERVATION_COLOURS.access, note: "仅保存 x/z：无世界 Y、不贴高、不吸附，也从未做过路线复核；access 记录只是观测。" }
    ];
    return { surface: state.surface, entries: entries, hiddenLayers: hidden.slice(0) };
  }
  /// Saved elevation range of the current layer, computed from the saved integer
  /// millimetre samples. An empty or non-finite layer reports why instead of
  /// falling back to zero.
  function rangeReading() {
    var surface = currentSurface();
    var heights = surface && surface.heightsMm ? surface.heightsMm : null;
    if (!heights || !heights.length) { return { surface: state.surface, available: false, reason: "the current layer carries no saved samples" }; }
    var min = null, max = null, count = 0, invalid = 0;
    for (var i = 0; i < heights.length; i++) {
      var value = heights[i];
      if (typeof value !== "number" || !isFinite(value)) { invalid++; continue; }
      if (min === null || value < min) { min = value; }
      if (max === null || value > max) { max = value; }
      count++;
    }
    if (min === null) { return { surface: state.surface, available: false, reason: "no finite saved sample", invalidCount: invalid }; }
    return {
      surface: state.surface, available: true,
      minMm: min, maxMm: max, minM: round6(min / 1000), maxM: round6(max / 1000),
      sampleCount: count, invalidCount: invalid, source: surface.source,
      note: "保存样本范围（不是城市海拔基准，也不是屏幕上所有像素的高度）"
    };
  }
  function rgbCss(colour) { return "rgb(" + Math.round(colour[0] * 255) + "," + Math.round(colour[1] * 255) + "," + Math.round(colour[2] * 255) + ")"; }
  // G30-R3: which legend entries stay in the compact summary. The ramp entry and its
  // note are detail-only; the four category entries carry the always-on semantics.
  var LEGEND_SUMMARY_ORDER = [0, 2, 3, 4];
  // G30-R3: the compact always-on summary. The drawing geometry is published on the
  // data seam so an independent reader can compute the drawn endpoints from the real
  // drawing constants instead of a hard-coded size. The full reading facts moved into
  // the inspect sidebar (see directionDetailHtml/legendDetailHtml/rangeDetailHtml), so
  // the bar stays one normal-flow row and the map keeps its contract area.
  var DIR_SUMMARY = { size: 40, centre: 20, scale: 10, labelGap: 6, labelDrop: 2, labelFont: 8, radius: 18, strokeWidth: 1.4 };
  var DIRECTION_NOTE = "X东·Y上·Z南 / 局部坐标，非真北";
  function directionSummaryHtml(direction) {
    if (!direction) { return '<span class="muted">方向：模型尚未加载</span>'; }
    var g = DIR_SUMMARY;
    var svg = '<svg class="dir" width="' + g.size + '" height="' + g.size + '" viewBox="0 0 ' + g.size + " " + g.size + '" role="img" aria-label="' + escapeHtml(direction.frameStatement) + '">';
    svg += '<circle cx="' + g.centre + '" cy="' + g.centre + '" r="' + g.radius + '" fill="none" stroke="#3a4350"/>';
    for (var i = 0; i < WORLDAXES.length; i++) {
      var axis = WORLDAXES[i], entry = direction.axes[axis.key];
      if (!entry || entry.degenerate) { continue; }
      var x2 = g.centre + entry.screenX * g.scale, y2 = g.centre + entry.screenY * g.scale;
      var lx = g.centre + entry.screenX * (g.scale + g.labelGap), ly = g.centre + entry.screenY * (g.scale + g.labelGap) + g.labelDrop;
      svg += '<line x1="' + g.centre + '" y1="' + g.centre + '" x2="' + round6(x2) + '" y2="' + round6(y2) + '" stroke="' + axis.colour + '" stroke-width="' + g.strokeWidth + '"/>';
      svg += '<text x="' + round6(lx) + '" y="' + round6(ly) + '" fill="' + axis.colour + '" font-size="' + g.labelFont + '" text-anchor="middle">' + axis.short + "</text>";
    }
    svg += "</svg>";
    // The compact summary states the degeneracy without a fabricated direction; the
    // axis names and the reason live in the details.
    var warn = direction.degenerate ? '<span class="warn">方向退化</span>' : "";
    return svg + '<span class="reading-note">' + escapeHtml(DIRECTION_NOTE) + "</span>" + warn;
  }
  function directionDetailHtml(direction) {
    if (!direction) { return '<div class="muted">方向：模型尚未加载</div>'; }
    return "<div>" + escapeHtml(direction.frameStatement) + "</div>" +
      '<div class="muted">相机 yaw ' + direction.camera.yaw + " / pitch " + direction.camera.pitch + "（" + escapeHtml(direction.camera.mode) + "）</div>" +
      '<div class="muted">轴向图由实时相机变换投影 ±X/±Z/±Y；旋转、俯视与切面都会同步。</div>' +
      // G32: the navigation help lives in this existing details block, so no new
      // permanent bar is added and the 45px summary and canvas area floors stay as
      // they were.
      '<div class="muted">拖动旋转；Shift+拖动平移；滚轮缩放。</div>' +
      (direction.degenerate ? '<div class="warn">方向退化：' + escapeHtml(direction.degenerateAxes.join("、")) + "（该轴在屏幕上不可分辨，不给伪方向）</div>" : "");
  }
  function legendItemHtml(entry) {
    var swatch = entry.kind === "ramp"
      ? '<span class="swatch ramp" style="background:linear-gradient(90deg,' + entry.from + "," + entry.to + ')"></span>'
      : '<span class="swatch" style="background:' + entry.swatch + '"></span>' + (entry.extraSwatch ? '<span class="swatch" style="background:' + entry.extraSwatch + '"></span>' : "");
    return '<span class="legend-item' + (entry.hidden ? " is-hidden" : "") + '">' + swatch + escapeHtml(entry.summaryLabel || entry.label) + (entry.hidden ? '<span class="muted">（已隐藏）</span>' : "") + "</span>";
  }
  function legendSummaryHtml(legend) {
    if (!legend) { return '<span class="muted">图例：模型尚未加载</span>'; }
    var html = '<div class="legend-row">';
    for (var k = 0; k < LEGEND_SUMMARY_ORDER.length; k++) {
      var entry = legend.entries[LEGEND_SUMMARY_ORDER[k]];
      if (entry) { html += legendItemHtml(entry); }
    }
    return html + "</div>";
  }
  function legendDetailHtml(legend) {
    if (!legend) { return '<div class="muted">图例：模型尚未加载</div>'; }
    var html = '<div class="legend-row">';
    for (var i = 0; i < legend.entries.length; i++) { html += legendItemHtml(legend.entries[i]); }
    html += "</div>";
    for (var j = 0; j < legend.entries.length; j++) {
      var entry = legend.entries[j];
      if (entry.note) { html += '<div class="muted">' + escapeHtml(entry.label) + "：" + escapeHtml(entry.note) + "</div>"; }
    }
    if (legend.hiddenLayers.length) { html += '<div class="muted">当前隐藏图层：' + escapeHtml(legend.hiddenLayers.join("、")) + "</div>"; }
    return html;
  }
  function rangeSummaryHtml(range) {
    if (!range) { return '<span class="muted">保存高程范围：模型尚未加载</span>'; }
    if (!range.available) { return '<span class="warn">保存样本 ' + escapeHtml(range.surface) + "：不可用（" + escapeHtml(range.reason) + "）</span>"; }
    return "<span>保存样本 " + escapeHtml(range.surface) + "：" + range.minM + "–" + range.maxM + " m</span>";
  }
  function rangeDetailHtml(range) {
    if (!range) { return '<div class="muted">保存高程范围：模型尚未加载</div>'; }
    if (!range.available) { return '<div class="warn">保存高程范围不可用：' + escapeHtml(range.reason) + (range.invalidCount ? "（非有限样本 " + range.invalidCount + " 个）" : "") + "</div>"; }
    return "<div>保存样本范围 <strong>" + escapeHtml(range.surface) + "</strong>：" + range.minM + " m … " + range.maxM + " m" +
      ' <span class="muted">（原始 ' + range.minMm + " … " + range.maxMm + " mm，" + range.sampleCount + " 个保存样本" + (range.invalidCount ? "，非有限 " + range.invalidCount + " 个" : "") + "）</span></div>" +
      '<div class="muted">' + escapeHtml(range.note) + " · 来源 " + escapeHtml(range.source) + "</div>";
  }
  function setDetailHtml(id, html) { var host = document.getElementById(id); if (host && host.innerHTML !== html) { host.innerHTML = html; } }
  function syncReadingDetailsFlag() {
    var button = document.getElementById("reading-details-open");
    var details = document.getElementById("reading-details");
    if (button && details) { button.setAttribute("aria-expanded", details.open === true ? "true" : "false"); }
  }
  /// G30-R3: the 读图详情 entry point. It switches to inspect and opens the reading
  /// details section in the existing sidebar; it never builds an overlay, never adds a
  /// second canvas and never reloads the model.
  function openReadingDetails() {
    var details = document.getElementById("reading-details");
    setViewMode("inspect");
    if (details) {
      details.open = true;
      state.readingDetailsOpen = true;
      if (details.scrollIntoView) { try { details.scrollIntoView({ block: "nearest" }); } catch (error) { details.scrollIntoView(); } }
    }
    syncReadingDetailsFlag();
    render();
  }

  function refreshHud() {
    if (!model) { return; }
    var status = model.status;
    var glInfo = document.getElementById("gl-info");
    if (glInfo) { glInfo.innerHTML = hudGlHtml(); }
    var counts = document.getElementById("counts");
    if (counts) { counts.innerHTML = hudCountsHtml(status); }
    var layerInfo = document.getElementById("layer-info");
    if (layerInfo) { layerInfo.innerHTML = hudLayerHtml(status); }
    // G30-R3: the compact summary is refreshed on every render exactly like the facts
    // panel; the full details are refreshed only when their text really changes, so a
    // render can never collapse a section the user opened or discard the panel scroll.
    var direction = directionReading(), legend = legendReading(), range = rangeReading();
    var directionHost = document.getElementById("reading-direction");
    if (directionHost) { directionHost.innerHTML = directionSummaryHtml(direction); }
    var legendHost = document.getElementById("reading-legend");
    if (legendHost) { legendHost.innerHTML = legendSummaryHtml(legend); }
    var rangeHost = document.getElementById("reading-range");
    if (rangeHost) { rangeHost.innerHTML = rangeSummaryHtml(range); }
    var detailDirection = directionDetailHtml(direction), detailLegend = legendDetailHtml(legend), detailRange = rangeDetailHtml(range);
    var detailSignature = detailDirection + "|" + detailLegend + "|" + detailRange;
    if (detailSignature !== state.readingDetailSignature) {
      state.readingDetailSignature = detailSignature;
      setDetailHtml("reading-detail-direction", detailDirection);
      setDetailHtml("reading-detail-legend", detailLegend);
      setDetailHtml("reading-detail-range", detailRange);
    }
    syncReadingDetailsFlag();
    state.readingRefreshes = (state.readingRefreshes || 0) + 1;
    var summaryText = { direction: hudText("reading-direction"), legend: hudText("reading-legend"), range: hudText("reading-range") };
    var detailText = { direction: hudText("reading-detail-direction"), legend: hudText("reading-detail-legend"), range: hudText("reading-detail-range") };
    state.reading = {
      direction: direction, legend: legend, range: range,
      summaryText: summaryText, detailText: detailText,
      detailsOpen: state.readingDetailsOpen === true,
      domText: {
        direction: summaryText.direction + " " + detailText.direction,
        legend: summaryText.legend + " " + detailText.legend,
        range: summaryText.range + " " + detailText.range
      }
    };
    state.hudRefreshes = (state.hudRefreshes || 0) + 1;
    state.hud = {
      surface: state.surface,
      surfaceSource: state.surfaceSource,
      surfaceMaterial: state.surfaceMaterial,
      overlayVisible: state.overlayVisible,
      overlayText: state.overlayVisible ? "shown" : "hidden",
      terrainIndexCount: state.terrainIndexCount,
      terrainTriangleCount: state.terrainTriangleCount,
      entityRoadTriangleCount: state.entityRoadTriangleCount,
      drawnTriangleCount: state.drawnTriangleCount,
      indexedDrawCalls: state.indexedDrawCalls,
      solidDrawCalls: state.solidDrawCalls,
      visibleLayers: state.visibleLayers,
      hiddenLayers: state.hiddenLayers,
      glInfoText: hudText("gl-info"),
      countsText: hudText("counts"),
      layerInfoText: hudText("layer-info")
    };
  }

  /// Boot-time full fill: refreshes the dynamic lines and then writes the static
  /// source/limitation blocks plus the saved-fact counters.
  function fillHud() {
    refreshHud();
    var status = model.status;
    var byCategory = status.categoryCounts || {};
    var entities = byCategory.entities || {}, roads = byCategory.roads || {};
    var nodes = byCategory.nodes || {}, access = byCategory.access || {};
    var evidence = document.getElementById("evidence");
    if (evidence) { evidence.innerHTML = hudEvidenceHtml(status); }
    var limitations = document.getElementById("limitations");
    if (limitations) { limitations.innerHTML = (model.limitations || []).map(function (text) { return "<li>" + escapeHtml(text) + "</li>"; }).join(""); }
    state.entityCount = entities.total; state.entitySolidCount = entities.solid;
    state.roadCount = roads.total; state.roadSolidCount = roads.solid;
    state.nodeCount = nodes.total; state.accessPathCount = access.total;
    state.observation2dObjectCount = status.observation2d ? status.observation2d.total : 0;
    state.observation2dNodes = status.observation2d ? status.observation2d.nodes : 0;
    state.observation2dAccess = status.observation2d ? status.observation2d.access : 0;
    state.heightlessObjectCount = status.heightlessObjectCount;
    state.heightlessByCategory = status.noHeight;
    state.notPositionedCount = status.notPositionedCount;
    state.notPositionedByCategory = status.notPositioned;
    state.notSnappedCount = status.notSnappedCount;
    state.notSnappedByCategory = status.notSnapped;
    state.snappedCount = status.snappedCount;
    state.constantWorldYUsed = status.constantWorldYUsed;
    state.snapping = status.snapping;
  }

  function boot() {
    var started = performance.now();
    // G29: a second boot inside one page (a reload of the model) is observable.
    state.pageLoads = (state.pageLoads || 0) + 1;
    var node = document.getElementById("render-model-3d");
    try {
      var raw = node.textContent.replace(/\/\*3D-MODEL-BEGIN\*\//g, "").replace(/\/\*3D-MODEL-END\*\//g, "");
      model = JSON.parse(raw);
      if (!model || !model.window || !model.surfaces) { throw new Error("3D model is missing window or surfaces"); }
      initGl();
      applyCameraPreset("default");
      state.modelLoaded = true;
      fillHud();
      renderSelection();
      var radios = document.getElementsByName("surface");
      for (var i = 0; i < radios.length; i++) {
        radios[i].addEventListener("change", function () { state.surface = this.value; render(); });
      }
      ["showEntities", "showRoads"].forEach(function (id) {
        var element = document.getElementById(id);
        if (element) { element.addEventListener("change", render); }
      });
      var observationToggle = document.getElementById("showObservation");
      if (observationToggle) { observationToggle.addEventListener("change", function () { state.overlayVisible = this.checked; render(); }); }
      var canvas = document.getElementById("gl");
      // G32: one active pointer, a mode locked at pointerdown, and a single exit that
      // every cancel path shares. A pan gesture never picks, and a Shift click that
      // never crosses the drag threshold leaves the selection alone.
      // G32-R1 option A: state.pointerGesture is the ONE source of truth for the active
      // gesture. There is deliberately no closure copy any more, because the old
      // pointerStart was only ever written and never reset, so after the first gesture
      // the page refused every new pointerdown and the stale gesture kept eating moves.
      canvas.addEventListener("pointerdown", function (event) {
        if (state.pointerGesture || event.button !== 0) { return; }
        var rect = canvas.getBoundingClientRect();
        state.pointerGesture = {
          pointerId: event.pointerId, mode: event.shiftKey ? "pan" : "rotate",
          x: event.clientX, y: event.clientY, moved: 0,
          yaw: state.camera.yaw, pitch: state.camera.pitch,
          target: [state.camera.target[0], state.camera.target[1], state.camera.target[2]],
          canvas: { width: rect.width, height: rect.height },
          endReason: null, cancelled: false, picked: false,
          panApplied: false, rejected: false
        };
        if (canvas.setPointerCapture) { try { canvas.setPointerCapture(event.pointerId); } catch (error) { /* ignore */ } }
      });
      canvas.addEventListener("pointermove", function (event) {
        var gesture = state.pointerGesture;
        if (!gesture || (event.pointerId !== undefined && gesture.pointerId !== undefined && event.pointerId !== gesture.pointerId)) { return; }
        var dx = event.clientX - gesture.x, dy = event.clientY - gesture.y;
        gesture.moved = Math.max(gesture.moved, Math.hypot(dx, dy));
        if (gesture.moved <= PAN_DRAG_THRESHOLD) { return; }
        if (gesture.mode === "pan") {
          applyPanStep(gesture, canvas, dx, dy);
          return;
        }
        state.camera.yaw = gesture.yaw - dx * 0.006;
        state.camera.pitch = clamp(gesture.pitch + dy * 0.005, -1.45, 1.5533);
        state.camera.mode = "free";
        render();
      });
      canvas.addEventListener("pointerup", function (event) {
        var active = state.pointerGesture;
        if (!active) { return; }
        var gesture = endPointerGesture("pointerup", event);
        if (canvas.releasePointerCapture) { try { canvas.releasePointerCapture(event.pointerId); } catch (error) { /* ignore */ } }
        if (gesture && gesture.mode === "pan") { gesture.picked = false; return; }
        if (!gesture || event.button !== 0 || active.moved > PAN_DRAG_THRESHOLD) { return; }
        var rect = canvas.getBoundingClientRect();
        pick(event.clientX - rect.left, event.clientY - rect.top);
        gesture.picked = true;
        var last = state.gestureLog[state.gestureLog.length - 1];
        if (last && last.endReason === "pointerup") { last.picked = true; }
      });
      canvas.addEventListener("pointercancel", function (event) { endPointerGesture("pointercancel", event); });
      canvas.addEventListener("lostpointercapture", function (event) { endPointerGesture("lostpointercapture", event); });
      window.addEventListener("blur", function (event) { endPointerGesture("blur", event); });
      // G33-R1: the overview keeps its own pointer handlers on its own element, so a gesture
      // that starts on the minimap can never enter the map state machine and a map gesture can
      // never move the overview. It never picks and never clears the selection; it only moves
      // camera.target.x/z. Ownership rules: while one pointer owns the drag every other
      // pointer's down/move/up/cancel is ignored, the internal state is cleared BEFORE the
      // capture is released (so the synchronous lostpointercapture cannot end a new drag), and
      // window blur clears the drag explicitly.
      var minimap = document.getElementById("minimap");
      if (minimap) {
        /// true only when this event belongs to the pointer that owns the active drag
        function minimapOwns(event) {
          var drag = state.minimapDrag;
          if (!drag) { return false; }
          if (event.pointerId === undefined || drag.pointerId === undefined) { return true; }
          return event.pointerId === drag.pointerId;
        }
        minimap.addEventListener("pointerdown", function (event) {
          if (event.button !== 0) { return; }
          if (state.minimapDrag) { return; }              // one active pointer only
          state.minimapDrag = { pointerId: event.pointerId, x: event.clientX, y: event.clientY, moved: 0, applied: false };
          if (minimap.setPointerCapture) { try { minimap.setPointerCapture(event.pointerId); } catch (error) { /* ignore */ } }
          var point = minimapPointFromEvent(minimap, event);
          state.minimapDrag.applied = applyMinimapJump(point.px, point.py, "pointerdown");
          event.preventDefault();
        });
        minimap.addEventListener("pointermove", function (event) {
          if (!minimapOwns(event)) { return; }
          var drag = state.minimapDrag;
          drag.moved = Math.max(drag.moved, Math.hypot(event.clientX - drag.x, event.clientY - drag.y));
          if (drag.moved <= MINIMAP_THRESHOLD) { return; }
          var point = minimapPointFromEvent(minimap, event);
          if (applyMinimapJump(point.px, point.py, "pointermove")) { drag.applied = true; }
          event.preventDefault();
        });
        minimap.addEventListener("pointerup", function (event) {
          if (!minimapOwns(event)) { return; }            // a foreign pointer can never end it
          var drag = state.minimapDrag;
          var point = minimapPointFromEvent(minimap, event);
          if (applyMinimapJump(point.px, point.py, "pointerup")) { drag.applied = true; }
          endMinimapDrag("pointerup");                    // clear the state FIRST
          if (minimap.releasePointerCapture) { try { minimap.releasePointerCapture(event.pointerId); } catch (error) { /* ignore */ } }
          event.preventDefault();
        });
        minimap.addEventListener("pointercancel", function (event) {
          if (!minimapOwns(event)) { return; }            // a foreign cancel is not our ending
          endMinimapDrag("pointercancel");                // cancelled is NOT a completed drag
          if (minimap.releasePointerCapture) { try { minimap.releasePointerCapture(event.pointerId); } catch (error) { /* ignore */ } }
        });
        minimap.addEventListener("lostpointercapture", function (event) {
          if (!minimapOwns(event)) { return; }
          endMinimapDrag("lostpointercapture");
        });
        window.addEventListener("blur", function () { endMinimapDrag("blur"); });
      }
      canvas.addEventListener("wheel", function (event) {
        event.preventDefault();
        zoomCamera(event.deltaY);
      }, { passive: false });
      var bind = function (id, action) { var element = document.getElementById(id); if (element) { element.addEventListener("click", action); } };
      bind("cam-default", function () { applyCameraPreset("default"); });
      bind("cam-oblique", function () { applyCameraPreset("oblique"); });
      bind("cam-top", function () { applyCameraPreset("top"); });
      bind("cam-focus", function () { applyCameraPreset("focus"); });
      bind("cam-reset", function () { applyCameraPreset("default"); });
      bind("cam-zoom-in", function () { zoomCamera(-160); });
      bind("cam-zoom-out", function () { zoomCamera(160); });
      bind("cam-rotate-left", function () { rotateCamera(-60, 0); });
      bind("cam-rotate-right", function () { rotateCamera(60, 0); });
      bind("cam-clear", clearSelection);
      bind("hud-toggle", function () { setHudCollapsed(!state.hudCollapsed); });
      // G29: the display-mode switch and the one-control expansion of the full
      // selection facts. Both are layout only; neither reloads the model.
      bind("mode-browse", function () { setViewMode("browse"); });
      bind("mode-inspect", function () { setViewMode("inspect"); });
      // G30-R3: the 读图详情 entry point and the manual toggle stay in sync with the
      // published flag, so a test can tell whether the details are open.
      bind("reading-details-open", openReadingDetails);
      var readingDetails = document.getElementById("reading-details");
      if (readingDetails) {
        readingDetails.addEventListener("toggle", function () {
          state.readingDetailsOpen = readingDetails.open === true;
          syncReadingDetailsFlag();
        });
      }
      var selectionHost = document.getElementById("selection");
      if (selectionHost) {
        selectionHost.addEventListener("toggle", function (event) {
          if (event.target && event.target.id === "selection-details") { state.selectionDetailsOpen = !!event.target.open; }
        }, true);
      }
      window.addEventListener("resize", function () { render(); });
      applyViewMode();
      render();
      state.loadMs = round6(performance.now() - started);
      fillHud();
    } catch (error) {
      state.loadMs = round6(performance.now() - started);
      fail("3D preview refused: " + error.message);
    }
  }

  // Local test seam for the executable viewer regression: it exposes the very
  // topology function the draw and pick paths use, so the JS harness can assert
  // legal sizes and explicit rejection of illegal ones. The harness also asserts
  // the real draw/pick chain (submitted index count and a real terrain hit), so
  // the seam is never the only evidence.
  window.__topology3d = function (width, depth, vertexCount) {
    return terrainTopology(width, depth, vertexCount);
  };
  /// G34-R1 controlled-experiment seam: a verifier may set the camera through
  /// `__PREVIEW3D_STATE__` and then trigger exactly one render through this hook. It performs no
  /// camera mutation of its own, so a controlled camera set cannot be silently moved by triggering
  /// a draw -- the verifier asserts the actual camera equals the requested one afterwards.
  window.__render3d = function () {
    render();
    return {
      yaw: state.camera.yaw, pitch: state.camera.pitch, distance: state.camera.distance,
      target: state.camera.target.slice(), mode: state.camera.mode
    };
  };
  window.__cameraLimits3d = function () { return cameraLimits(); };
  window.__terrainRuntime3d = function () {
    return {
      surface: state.surface,
      surfaceVertexCount: state.surfaceVertexCount,
      terrainIndexCount: state.terrainIndexCount,
      terrainTriangleCount: state.terrainTriangleCount,
      modelDeclaredTerrainTriangles: state.modelDeclaredTerrainTriangles,
      modelDeclaredTerrainVertices: state.modelDeclaredTerrainVertices,
      entityRoadTriangleCount: state.entityRoadTriangleCount,
      drawnTriangleCount: state.drawnTriangleCount,
      submittedIndices: state.submittedIndices,
      indexedDrawCalls: state.indexedDrawCalls,
      solidDrawCalls: state.solidDrawCalls,
      terrainDrawnWithIndexBuffer: state.terrainDrawnWithIndexBuffer,
      terrainTopologyValidated: state.terrainTopologyValidated,
      terrainIndicesBySurface: state.terrainIndicesBySurface,
      terrainTrianglesBySurface: state.terrainTrianglesBySurface,
      hudRefreshes: state.hudRefreshes,
      renderCount: state.renderCount,
      hud: state.hud,
      reading: state.reading,
      viewMode: state.viewMode,
      modeSwitches: state.modeSwitches,
      layoutSyncs: state.layoutSyncs,
      layout: state.layout,
      selectionObjectKey: state.selection ? state.selection.objectKey : null,
      pageLoads: state.pageLoads
    };
  };
  /// G30: the reading-aid seam. The acceptance driver compares these values with
  /// its own independent computation from the camera snapshot and the model, so
  /// the reading is never checked against itself.
  window.__reading3d = function () {
    var direction = directionReading(), legend = legendReading(), range = rangeReading();
    var summaryText = { direction: hudText("reading-direction"), legend: hudText("reading-legend"), range: hudText("reading-range") };
    var detailText = { direction: hudText("reading-detail-direction"), legend: hudText("reading-detail-legend"), range: hudText("reading-detail-range") };
    return {
      direction: direction, legend: legend, range: range,
      directionSummaryGeometry: DIR_SUMMARY,
      summaryText: summaryText, detailText: detailText,
      domText: {
        direction: summaryText.direction + " " + detailText.direction,
        legend: summaryText.legend + " " + detailText.legend,
        range: summaryText.range + " " + detailText.range
      },
      detailsOpen: state.readingDetailsOpen === true,
      readingRefreshes: state.readingRefreshes
    };
  };
  window.__pick3d = function (x, y) {
    pick(x, y);
    return { pickMethod: state.pickMethod, pickObject: state.pickObject, selection: state.selection, terrainHit: state.terrainHit, roadHit: state.roadHit, hiddenLayers: state.hiddenLayers };
  };
  // G32: the pure pan math is published so a test can compute an independent
  // expectation without driving the pointer state machine.
  window.__panTarget3d = function (camera, canvasSize, startTarget, dx, dy) {
    return panTargetFromDrag(camera, canvasSize, startTarget, dx, dy);
  };
  window.__panBasis3d = function (yaw, pitch) { return panBasis(yaw, pitch); };
  // G33: the minimap pure functions are published so the acceptance driver can
  // compute its own independent expectation (it never checks the product against
  // itself), and a read-only snapshot tells an overview jump from a map gesture.
  window.__minimapWorldToPx3d = function (win, size, x, z) { return minimapWorldToPx(win, size, x, z); };
  window.__minimapPxToWorld3d = function (win, size, px, py) { return minimapPxToWorld(win, size, px, py); };
  window.__minimapFootprint3d = function (camera, canvasSize, win, planeY) { return minimapFootprint(camera, canvasSize, win, planeY); };
  window.__minimapTarget3d = function (win, size, px, py, startTarget) { return minimapTargetFromPx(win, size, px, py, startTarget); };
  window.__minimap3d = function () { return state.minimap; };
  // G34: the visibility snapshot and the pure functions behind it, published read-only so an
  // independent oracle can recompute every expectation instead of trusting the drawing.
  // G34-R4: this returns the FROZEN snapshot built by the last render. It does NOT recompute a grid,
  // so a reader automatically gets the counts, the cell list and the drawn set from ONE evaluation.
  window.__minimapVisibility3d = function () { return state.minimapVisibility; };
  // G34-R4: the grid hook is an INDEPENDENT DIAGNOSTIC. It runs its own evaluation with whatever
  // inputs the caller passes, so its numbers describe that call and NOT the rendered frame. It is
  // labelled so it cannot be quoted as "the same frame" by mistake.
  window.__minimapVisibilityGrid3d = function (camera, canvasSize, win, footprint) {
    var g = minimapVisibilityGrid(camera, canvasSize, win, footprint);
    g.independent = true;
    g.fromRenderedFrame = false;
    g.independentNote = "this is a separate evaluation for a caller-supplied camera/canvas; it is NOT the rendered frame. For the rendered frame read __minimapVisibility3d().";
    return g;
  };
  window.__minimapTerrainHeight3d = function (surfaceName, x, z) { return occlusionTerrainHeight(occlusionIndexFor(surfaceName), x, z); };
  window.__minimapOcclusionProbe3d = function (surfaceName, eye, x, z, referenceY) { return occlusionClassify(occlusionIndexFor(surfaceName), eye, x, z, referenceY); };
  /// G34-R2 read-only diagnostic seam: the FULL detail of one classification, including which path
  /// decided it and the numbers behind it.
  window.__minimapOcclusionDetail3d = function (surfaceName, eye, x, z, referenceY) {
    var index = occlusionIndexFor(surfaceName);
    if (!index) { return null; }
    var perf = { candidateTriangles: 0, intersectionTests: 0, dedupedTriangles: 0, prefilterCleared: 0 };
    // G34-R3: the probe must NOT reuse a fixed marker. A hard-coded marker collides with the grid's
    // own epochs and pollutes the main run; it draws from the same monotonic source instead.
    var probeEpoch = nextOcclusionEpoch(index.dedupeStamp);
    var detail = occlusionClassifyDetailed(index, eye, x, z, referenceY, perf, probeEpoch, index.dedupeStamp, index.candidates);
    return { klass: detail.klass, reason: detail.reason, sMetres: detail.sMetres === null ? null : round6(detail.sMetres), candidateCount: detail.candidateCount, testedCount: detail.testedCount, dedupedCount: detail.dedupedCount, prefilterCleared: detail.prefilterCleared === true, perf: perf };
  };
  window.__minimapVisibilityConstants3d = function () { return { resolution: MINIMAP_VISIBILITY_RESOLUTION, liftM: MINIMAP_VISIBILITY_LIFT_M, epsilonM: MINIMAP_VISIBILITY_EPS_M, bucketM: MINIMAP_VISIBILITY_BUCKET_M }; };
  /// G34-R2 read-only diagnostic seam: report how a single sight line is judged by the conservative
  /// pre-filter, so an auditor can see WHY a cell took the fast path or the exact path.
  window.__minimapPreFilterProbe3d = function (surfaceName, eye, x, z, referenceY) {
    var index = occlusionIndexFor(surfaceName);
    if (!index) { return null; }
    var target = [x, referenceY + MINIMAP_VISIBILITY_LIFT_M, z];
    var d = sub3(target, eye);
    var len = Math.hypot(d[0], d[1], d[2]);
    if (!(len > 0)) { return null; }
    var dir = [d[0] / len, d[1] / len, d[2] / len];
    var bounds = occlusionBuildHeightBounds(index);
    return {
      lengthMetres: round6(len),
      clearsTerrain: occlusionLineClearsTerrain(index, bounds, eye, dir, len),
      boundsCells: bounds.length,
      finiteBounds: (function () { var n = 0; for (var i = 0; i < bounds.length; i++) { if (bounds[i] !== -Infinity) { n++; } } return n; })(),
      maxBound: (function () { var m = -Infinity; for (var i = 0; i < bounds.length; i++) { if (bounds[i] > m) { m = bounds[i]; } } return m === -Infinity ? null : round6(m); })()
    };
  };
  window.__setSurface3d = function (name) {    state.surface = name;
    var radios = document.getElementsByName("surface");
    for (var i = 0; i < radios.length; i++) { radios[i].checked = radios[i].value === name; }
    render();
  };

  if (document.readyState === "loading") { document.addEventListener("DOMContentLoaded", boot); } else { boot(); }
})();
