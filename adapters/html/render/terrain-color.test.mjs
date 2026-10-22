import assert from 'node:assert/strict'
import test from 'node:test'

import { makePolylineRibbonGeometry, srgbChannelToLinear } from './terrain.js'

test('terrain vertex colors are converted from sRGB before entering Three', () => {
  assert.equal(srgbChannelToLinear(0), 0)
  assert.equal(srgbChannelToLinear(1), 1)
  assert.ok(Math.abs(srgbChannelToLinear(.5) - .2140411405) < 1e-9)
})

test('terrain vertex color conversion clamps malformed channels', () => {
  assert.equal(srgbChannelToLinear(-.2), 0)
  assert.equal(srgbChannelToLinear(1.2), 1)
})

test('road ribbons keep the baked design surface instead of resampling terrain', () => {
  class Attribute {
    constructor(array) {
      this.array = array
    }
  }
  class Geometry {
    setAttribute(name, value) {
      this[name] = value
    }
    setIndex() {}
    computeVertexNormals() {}
  }
  const THREE = {
    BufferGeometry: Geometry,
    Float32BufferAttribute: Attribute,
  }
  const points = [
    { x: 0, z: 0, designHeightM: 10 },
    { x: 20, z: 0, designHeightM: 14 },
  ]
  const geometry = makePolylineRibbonGeometry(
    points,
    2,
    () => 40,
    THREE,
    .9,
    0,
    0,
  )
  const y = geometry.position.array.filter((_, index) => index % 3 === 1)
  assert.ok(y.every(value => value >= 10.9 && value <= 14.935))
  assert.ok(y.some(value => Math.abs(value - 10.9) < 1e-9))
  assert.ok(y.some(value => Math.abs(value - 14.9) < 1e-9))
})
