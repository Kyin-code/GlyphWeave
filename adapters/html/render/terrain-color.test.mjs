import assert from 'node:assert/strict'
import test from 'node:test'

import { srgbChannelToLinear } from './terrain.js'

test('terrain vertex colors are converted from sRGB before entering Three', () => {
  assert.equal(srgbChannelToLinear(0), 0)
  assert.equal(srgbChannelToLinear(1), 1)
  assert.ok(Math.abs(srgbChannelToLinear(.5) - .2140411405) < 1e-9)
})

test('terrain vertex color conversion clamps malformed channels', () => {
  assert.equal(srgbChannelToLinear(-.2), 0)
  assert.equal(srgbChannelToLinear(1.2), 1)
})
