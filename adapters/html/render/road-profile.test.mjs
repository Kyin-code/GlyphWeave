import assert from 'node:assert/strict'
import test from 'node:test'

import { gradedRoadGeometry } from './road-profile.js'

function road(width, length = 64) {
  const samples = Array.from({ length: length + 1 }, (_, i) => i * 10)
  return {
    entityId: `road-${width}`,
    kind: 'road',
    widthM: length,
    depthM: width,
    scale: 1,
    worldX: length / 2,
    worldZ: 100,
    grounding: {
      roadProfile: {
        version: 1,
        start: [0, 100],
        end: [length, 100],
        elevationsMm: samples,
      },
    },
  }
}

test('graded road geometry consumes every authored hierarchy width', () => {
  for (const width of [8, 12, 16]) {
    const geometry = gradedRoadGeometry(road(width))
    assert.equal(geometry.positions.length, (64 + 1) * 2 * 3)
    assert.equal(geometry.positions[2], -width / 2)
    assert.equal(geometry.positions[5], width / 2)
  }
})

test('graded road geometry rejects widths outside the Rust contract', () => {
  for (const width of [3, 33]) {
    assert.throws(
      () => gradedRoadGeometry(road(width)),
      /Invalid Rust road profile domain/,
    )
  }
})
