// Static preview wind contract.
//
// The world generator remains deterministic; the HTML acceptance view currently
// disables wind so that terrain, bridge geometry, placement and occlusion can be
// inspected without animated vegetation masking defects. Re-enable motion only
// behind an explicit future preview setting.
export function createWorldWind(THREE) {
  return {
    uTime: { value: 0 },
    uWindDir: { value: new THREE.Vector2(Math.cos(THREE.MathUtils.degToRad(20)), Math.sin(THREE.MathUtils.degToRad(20))) },
    uWindStrength: { value: 0 },
    uWindSpeed: { value: 0 },
    uWindScale: { value: .35 },
    uGust: { value: 0 },
  }
}
