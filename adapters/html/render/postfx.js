// Cinematic post-processing stack — ported from achrefelouafi/GrassSystemThreeJS
// (MIT). Render -> Depth of Field -> Bloom -> tone map/sRGB -> Film grade
// (chromatic aberration, contrast/saturation, vignette, animated grain).
// Accepts the THREE module + addons (EffectComposer etc.) as parameters so the
// CDN dynamic-import pipeline in app.js can feed them in.
export function createPostFX({ THREE, addons, renderer, scene, camera, samples = 2 }) {
  const { EffectComposer, RenderPass, BokehPass, UnrealBloomPass, ShaderPass, OutputPass } = addons

  const FilmGradeShader = {
    uniforms: {
      tDiffuse: { value: null },
      uTime: { value: 0 },
      uVignette: { value: 0.12 },
      uVignetteSize: { value: 0.5 },
      uGrain: { value: 0.0 },
      uChroma: { value: 0.0 },
      uContrast: { value: 1.0 },
      uSaturation: { value: 1.05 },
    },
    vertexShader: /* glsl */ `
      varying vec2 vUv;
      void main() {
        vUv = uv;
        gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
      }
    `,
    fragmentShader: /* glsl */ `
      uniform sampler2D tDiffuse;
      uniform float uTime, uVignette, uVignetteSize, uGrain, uChroma, uContrast, uSaturation;
      varying vec2 vUv;

      float rand(vec2 co) {
        return fract(sin(dot(co, vec2(12.9898, 78.233))) * 43758.5453);
      }

      void main() {
        vec2 dir = vUv - 0.5;

        // Radial chromatic aberration — stronger toward the frame edges.
        float ca = uChroma * dot(dir, dir) * 4.0;
        vec3 col;
        col.r = texture2D(tDiffuse, vUv - dir * ca).r;
        col.g = texture2D(tDiffuse, vUv).g;
        col.b = texture2D(tDiffuse, vUv + dir * ca).b;

        // Contrast + saturation grade.
        col = (col - 0.5) * uContrast + 0.5;
        float luma = dot(col, vec3(0.299, 0.587, 0.114));
        col = mix(vec3(luma), col, uSaturation);

        // Vignette.
        float vig = smoothstep(0.85, uVignetteSize, length(dir));
        col *= 1.0 - vig * uVignette;

        // Animated film grain.
        float g = rand(vUv + fract(uTime)) - 0.5;
        col += g * uGrain;

        gl_FragColor = vec4(col, 1.0);
      }
    `,
  }

  const w = window.innerWidth
  const h = window.innerHeight
  const maxSamples = renderer.capabilities.maxSamples

  // MSAA can only happen inside the composer — the renderer's own antialias
  // flag is bypassed once we render through render targets.
  function makeRenderTarget(n) {
    const dpr = renderer.getPixelRatio()
    const rt = new THREE.WebGLRenderTarget(
      Math.floor(window.innerWidth * dpr),
      Math.floor(window.innerHeight * dpr),
      { type: THREE.HalfFloatType }
    )
    rt.samples = Math.min(n, maxSamples)
    return rt
  }

  const composer = new EffectComposer(renderer, makeRenderTarget(samples))
  composer.addPass(new RenderPass(scene, camera))

  const bokeh = new BokehPass(scene, camera, {
    focus: 600,
    aperture: 0.0008,
    maxblur: 0.004,
  })
  bokeh.enabled = false // opt-in (DoF is costly; enable for street/explore)
  composer.addPass(bokeh)

  const bloom = new UnrealBloomPass(new THREE.Vector2(w, h), 0.15, 0.8, 0.72)
  composer.addPass(bloom)

  composer.addPass(new OutputPass())

  const grade = new ShaderPass(FilmGradeShader)
  composer.addPass(grade) // last -> renders to screen, display space

  function setSize(width, height) {
    composer.setSize(width, height)
    bloom.setSize(width, height)
  }
  setSize(w, h)

  function setSamples(n) {
    composer.reset(makeRenderTarget(n))
    setSize(window.innerWidth, window.innerHeight)
  }

  return {
    composer,
    bokeh,
    bloom,
    grade,
    maxSamples,
    setSize,
    setSamples,
    render(dt) {
      grade.uniforms.uTime.value += dt
      composer.render()
    },
  }
}
