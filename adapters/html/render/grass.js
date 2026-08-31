// GPU-instanced stylized grass blades — ported from achrefelouafi/GrassSystemThreeJS
// (MIT). Blades are placed on the CPU (base glued to our pre-generated terrain
// height via `heightAt`) and fully animated on the GPU: resting circular-arc
// curl, world-coherent wind gust + per-blade flutter, tip backlight
// translucency, and a coverage mask (patchy meadow) driven by the same noise
// family as the terrain texture. One InstancedBufferGeometry = one draw call
// per field.
//
// Blade-height presets are Kyin-tuned in Soil Studio (02-grass-system) and
// mirrored from render/presets/materials.js (grassPresets) — keep in sync.

// 浅草 / 中草 / 高草 (Kyin, 2026-08-31)
const lowPres = { bladeHeight: 0.2, bladeWidth: 0.02, curl: 0.25 }
const midPres = { bladeHeight: 0.78, bladeWidth: 0.042, curl: 0.48 }
const highPres = { bladeHeight: 1.54, bladeWidth: 0.05, curl: 0.73 }

export function makeGrassBlades(THREE, worldX, worldZ, width, depth, heightAt, count, salt, focusX, focusZ, options = {}) {
  const segments = 5
  const maxCount = Math.max(200, count)
  const positions = []
  const normals = []
  const indices = []
  for (let j = 0; j <= segments; j++) {
    const t = j / segments
    positions.push(-0.5, t, 0, 0.5, t, 0)
    normals.push(0, 0, 1, 0, 0, 1)
    if (j < segments) {
      const a = j * 2
      indices.push(a, a + 1, a + 3, a, a + 3, a + 2)
    }
  }
  const geometry = new THREE.InstancedBufferGeometry()
  geometry.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3))
  geometry.setAttribute('normal', new THREE.Float32BufferAttribute(normals, 3))
  geometry.setIndex(indices)

  const iWorld = new Float32Array(maxCount * 2)
  const iBaseY = new Float32Array(maxCount)
  const iYaw = new Float32Array(maxCount)
  const iHeight = new Float32Array(maxCount)
  const iWidth = new Float32Array(maxCount)
  const iPhase = new Float32Array(maxCount)
  const iCurlVar = new Float32Array(maxCount)
  const iColorVar = new Float32Array(maxCount)

  // Coverage mask: world-space noise decides where grass grows (patchy meadow),
  // AND a strong focus-density falloff concentrates blades near the camera
  // focus: rejection sampling with cubic distance decay puts ~90% of blades
  // inside the visible ~150m ring, so the foreground reads as dense meadow
  // while distant blades (sub-pixel anyway) cost almost nothing.
  //
  // Three-height layering (Kyin-tuned presets): low grass fills damp hollows,
  // mid grass is the meadow main, high grass crowns rises — giving the field a
  // natural depth gradient instead of one uniform blade height.
  let placed = 0
  let guard = 0
  const fx = focusX ?? (worldX + width / 2), fz = focusZ ?? (worldZ + depth / 2)
  const maxR = Math.max(width, depth) * .55
  const focusR = Math.min(options.focusR ?? 55, maxR) // dense core radius
  const farR = Math.min(options.farR ?? 220, maxR)     // sparse transition edge
  // Blade-height zones by terrain height: sample the range once so the layering
  // is consistent across the whole field.
  let hMin = Infinity, hMax = -Infinity
  for (let s = 0; s < 200; s++) {
    const sx = worldX + rnd(s, 1, salt) * width
    const sz = worldZ + rnd(s, 2, salt) * depth
    const hh = heightAt(sx, sz)
    if (hh < hMin) hMin = hh
    if (hh > hMax) hMax = hh
  }
  const hRange = Math.max(1e-3, hMax - hMin)
  while (placed < maxCount && guard < maxCount * 120) {
    guard++
    const x = worldX + rnd(guard, 0, salt) * width
    const z = worldZ + rnd(guard, 1, salt) * depth
    const cover = fallbackFractal(x * .0022 + salt * .013, z * .0022 - salt * .017) * .5 + .5
    if (cover < .3) continue
    // Distance falloff: dense within focusR, gentle square falloff out to farR.
    const d = Math.hypot(x - fx, z - fz)
    const keep = d < focusR ? 1 : d < farR ? Math.pow((farR - d) / (farR - focusR), 2) : 0
    if (rnd(guard, 8, salt) > keep) continue
    const gy = heightAt(x, z)
    iWorld[placed * 2] = x
    iWorld[placed * 2 + 1] = z
    iBaseY[placed] = gy
    iYaw[placed] = rnd(guard, 4, salt) * Math.PI * 2
    // Height zone from terrain elevation: low in hollows, high on rises, with a
    // deterministic per-blade jitter so the boundaries stay soft.
    const elev = (gy - hMin) / hRange
    const jitter = rnd(guard, 9, salt) * .3
    const zone = elev + jitter
    // Broad layering so all three heights appear: lows in hollows, mids on the
    // flats (the steppe's dominant cover), highs scattered on rises and damp
    // patches. The thresholds give each tier a fair share.
    const pres = zone < .3 ? lowPres : zone < .78 ? midPres : highPres
    // High grass gets a bigger height scatter so it pokes through as tufts.
    const varScale = pres === highPres ? .7 + rnd(guard, 2, salt) * .9 : .85 + rnd(guard, 2, salt) * .3
    iHeight[placed] = pres.bladeHeight * varScale
    iWidth[placed] = pres.bladeWidth * (1.2 - rnd(guard, 3, salt) * .4)
    iPhase[placed] = rnd(guard, 5, salt) * Math.PI * 2
    iCurlVar[placed] = pres.curl * (.7 + rnd(guard, 6, salt) * .6)
    iColorVar[placed] = rnd(guard, 7, salt)
    placed++
  }
  const used = placed
  geometry.setAttribute('iWorld', new THREE.InstancedBufferAttribute(iWorld, 2))
  geometry.setAttribute('iBaseY', new THREE.InstancedBufferAttribute(iBaseY, 1))
  geometry.setAttribute('iYaw', new THREE.InstancedBufferAttribute(iYaw, 1))
  geometry.setAttribute('iHeight', new THREE.InstancedBufferAttribute(iHeight, 1))
  geometry.setAttribute('iWidth', new THREE.InstancedBufferAttribute(iWidth, 1))
  geometry.setAttribute('iPhase', new THREE.InstancedBufferAttribute(iPhase, 1))
  geometry.setAttribute('iCurlVar', new THREE.InstancedBufferAttribute(iCurlVar, 1))
  geometry.setAttribute('iColorVar', new THREE.InstancedBufferAttribute(iColorVar, 1))
  geometry.instanceCount = used
  geometry.boundingSphere = new THREE.Sphere(new THREE.Vector3(worldX + width / 2, 0, worldZ + depth / 2), Math.max(width, depth))

  const uniforms = {
    time: { value: 0 },
    uWindDir: { value: new THREE.Vector2(1, .35).normalize() },
    uWindStrength: { value: .5 },
    uWindSpeed: { value: 1.8 },
    uWindScale: { value: .35 },
    uGust: { value: .6 },
    uCurl: { value: 1.0 },
    uBladeHeight: { value: 1.0 },
    uBladeWidth: { value: 1.0 },
    uColorBase: { value: new THREE.Color('#33421b') },
    uColorTip: { value: new THREE.Color('#9bc24a') },
    uColorVarAmt: { value: .5 },
    uTranslucency: { value: 1.1 },
    uSunDir: { value: new THREE.Vector3(-.45, .82, .3).normalize() },
    uSunColor: { value: new THREE.Color('#ffd9a0') },
    uSunIntensity: { value: 2.6 },
    uSkyColor: { value: new THREE.Color('#fff3d6') },
    uGroundColor: { value: new THREE.Color('#8a7a52') },
    uCameraPos: { value: new THREE.Vector3() },
    fogColor: { value: new THREE.Color('#9cb28c') },
    fogNear: { value: 1400 },
    fogFar: { value: 6000 },
  }

  const material = new THREE.ShaderMaterial({
    uniforms,
    side: THREE.DoubleSide,
    vertexShader: `
      attribute vec2 iWorld;
      attribute float iBaseY;
      attribute float iYaw;
      attribute float iHeight;
      attribute float iWidth;
      attribute float iPhase;
      attribute float iCurlVar;
      attribute float iColorVar;
      uniform float time;
      uniform vec2 uWindDir;
      uniform float uWindStrength;
      uniform float uWindSpeed;
      uniform float uWindScale;
      uniform float uGust;
      uniform float uCurl;
      uniform float uBladeHeight;
      uniform float uBladeWidth;
      varying float vT;
      varying float vColorVar;
      varying vec3 vWorldPos;
      varying vec3 vNormal;

      void main() {
        float t = position.y;
        float side = position.x;
        float h = uBladeHeight * iHeight;
        float w = uBladeWidth * iWidth * (1.0 - t) * (0.7 + 0.3 * (1.0 - t));

        // Resting curl as a circular arc of total angle A (exact normals, stable
        // at A -> 0).
        float A = uCurl * iCurlVar;
        float yA, zA, Tc, Ts;
        if (abs(A) > 0.001) {
          yA = h * sin(A * t) / A;
          zA = h * (1.0 - cos(A * t)) / A;
          Tc = cos(A * t); Ts = sin(A * t);
        } else {
          yA = h * t; zA = 0.0; Tc = 1.0; Ts = 0.0;
        }
        vec3 pLocal = vec3(side * w, yA, zA);
        vec3 nLocal = vec3(0.0, -Ts, Tc);

        // Yaw around Y by the per-blade heading.
        float cy = cos(iYaw), sy = sin(iYaw);
        vec3 pR = vec3(pLocal.x * cy + pLocal.z * sy, pLocal.y, -pLocal.x * sy + pLocal.z * cy);
        vec3 nR = vec3(nLocal.x * cy + nLocal.z * sy, nLocal.y, -nLocal.x * sy + nLocal.z * cy);

        // Coherent wind: a shared travelling gust + fine per-blade flutter,
        // pushing along the wind direction, strongest at the tip.
        float gph = dot(iWorld, uWindDir) * uWindScale + time * uWindSpeed + iPhase;
        float gust = sin(gph) * 0.6 + sin(gph * 0.5 + 1.7) * 0.4;
        float flutter = sin(time * 8.0 + iPhase * 3.0) * 0.15 * uGust;
        float sway = (gust + flutter) * uWindStrength;
        vec2 windOff = uWindDir * sway * (t * t);

        // Base glued to the terrain height on the CPU; add curl + wind on top.
        vec3 gPos = vec3(iWorld.x + pR.x + windOff.x,
                         iBaseY + pR.y,
                         iWorld.y + pR.z + windOff.y);
        vT = t;
        vColorVar = iColorVar;
        vWorldPos = gPos;
        vNormal = normalize(nR);
        gl_Position = projectionMatrix * modelViewMatrix * vec4(gPos, 1.0);
      }
    `,
    fragmentShader: `
      varying float vT;
      varying float vColorVar;
      varying vec3 vWorldPos;
      varying vec3 vNormal;
      uniform vec3 uColorBase;
      uniform vec3 uColorTip;
      uniform float uColorVarAmt;
      uniform float uTranslucency;
      uniform vec3 uSunDir;
      uniform vec3 uSunColor;
      uniform float uSunIntensity;
      uniform vec3 uSkyColor;
      uniform vec3 uGroundColor;
      uniform vec3 uCameraPos;
      uniform vec3 fogColor;
      uniform float fogNear;
      uniform float fogFar;

      void main() {
        float gradient = smoothstep(0.08, 1.0, vT);
        vec3 gcol = mix(uColorBase, uColorTip, gradient);
        gcol *= mix(1.0 - uColorVarAmt, 1.0 + uColorVarAmt, vColorVar);
        // Slight per-blade hue scatter (yellow-green to blue-green) so the
        // field reads as varied living grass, not one flat tone.
        float hueShift = (vColorVar - 0.5) * 0.35;
        gcol = gcol * (1.0 + hueShift * vec3(-0.25, 0.35, -0.3));
        // Base occlusion: darker at the roots.
        gcol *= mix(0.5, 1.0, smoothstep(0.0, 0.35, vT));

        vec3 N = normalize(vNormal);
        vec3 L = normalize(uSunDir);
        // Hemisphere-ish diffuse: sky light from above, warm ground bounce below.
        float sky = max(N.y, 0.0);
        float ground = max(-N.y, 0.0);
        vec3 hemi = uSkyColor * sky + uGroundColor * ground;
        // Flattened diffuse so per-ribbon shimmer doesn't flicker.
        float diffuse = 0.35 + 0.65 * max(dot(vec3(0.0, 1.0, 0.0), L), 0.0);
        vec3 lit = gcol * (hemi * .5 + uSunColor * uSunIntensity * diffuse);

        // Backlight translucency: strongest looking through a thin blade at the
        // sun (Journey-style rim glow).
        vec3 V = normalize(uCameraPos - vWorldPos);
        float back = pow(max(dot(V, -L), 0.0), 2.0);
        float edgeOn = 1.0 - abs(dot(N, L));
        lit += uColorTip * back * edgeOn * vT * uTranslucency * uSunColor * uSunIntensity;

        // Distance fog (matches the scene Fog).
        float dist = length(uCameraPos - vWorldPos);
        float fog = clamp((dist - fogNear) / (fogFar - fogNear), 0.0, 1.0);
        gl_FragColor = vec4(mix(lit, fogColor, fog), 1.0);
      }
    `,
  })

  const mesh = new THREE.Mesh(geometry, material)
  mesh.frustumCulled = false
  mesh.userData.vegetation = true
  return {
    mesh,
    material,
    uniforms,
    count: used,
    update(t) {
      uniforms.time.value = t
    },
  }
}

// Deterministic per-instance RNG (matches app.js / vegetation.js style).
function rnd(a, b, s) {
  let h = Math.imul(a + s * 1013904223, 2654435761) ^ Math.imul(b + s * 340573321, 1597334677)
  h = (h ^ (h >> 13)) * 1274126177
  h = h ^ (h >> 16)
  return ((h >>> 0) % 100000) / 100000
}

// Local fBm matching core/shared.js fractalNoise, so this module needs no
// import graph at runtime (keeps CDN dynamic-load compatibility).
function hashN(x, z) {
  const value = Math.sin(x * 127.1 + z * 311.7) * 43758.5453
  return value - Math.floor(value)
}
function smoothN(x, z) {
  const x0 = Math.floor(x); const z0 = Math.floor(z)
  const tx = x - x0; const tz = z - z0
  const sx = tx * tx * (3 - 2 * tx); const sz = tz * tz * (3 - 2 * tz)
  const a = hashN(x0, z0); const b = hashN(x0 + 1, z0)
  const c = hashN(x0, z0 + 1); const d = hashN(x0 + 1, z0 + 1)
  return (a + (b - a) * sx) * (1 - sz) + (c + (d - c) * sx) * sz
}
function fallbackFractal(x, z) {
  return smoothN(x, z) * .58 + smoothN(x * 2.7, z * 2.7) * .29 + smoothN(x * 7.1, z * 7.1) * .13
}
