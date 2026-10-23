import assert from 'node:assert/strict'
import { test } from 'node:test'

import { resolveOrbitButtonModes } from './camera.js'

test('left drag pans every camera preset by default', () => {
  assert.deepEqual(resolveOrbitButtonModes(), {
    left: 'pan',
    middle: 'pan',
    right: 'rotate',
  })
})

test('Alt+left always selects orbit input', () => {
  assert.deepEqual(resolveOrbitButtonModes({ altKey: true }), {
    left: 'rotate',
    middle: 'pan',
    right: 'rotate',
  })
})
