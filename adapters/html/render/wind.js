// One world-space wind field shared by grass, trees and future shrubs.
// Every vegetation shader receives these same uniform objects, so gust fronts
// travel coherently instead of each system inventing its own clock/direction.
export function createWorldWind(THREE) {
  return {
    uTime: { value: 0 },
    uWindDir: { value: new THREE.Vector2(.82, .57).normalize() },
    uWindStrength: { value: .5 },
    uWindSpeed: { value: 1.55 },
    uWindScale: { value: .22 },
    uGust: { value: .6 },
  }
}
