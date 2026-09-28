// This test uses the repository's existing bundled Three implementation, not
// a new dependency or downloaded asset. Run: node --test scripts/planned-home.test.mjs
import { test } from 'node:test'
import assert from 'node:assert/strict'
import * as THREE from '../adapters/html/vendor/three/build/three.module.js'
import { makePlannedResidentialHome } from '../adapters/html/render/buildings.js'
for(const yaw of [0,90,180,270]) {
  test(`planned home footprint and entrance yaw ${yaw}`,()=>{
    const entity={entityId:'planned.home',widthM:24,depthM:24,heightM:8,worldX:500,worldZ:400,worldY:3}
    const group=makePlannedResidentialHome(entity,THREE)
    group.rotation.y=THREE.MathUtils.degToRad(yaw)
    group.updateMatrixWorld(true)
    const box=new THREE.Box3().setFromObject(group)
    assert(Math.abs(box.min.y-3)<1e-6);assert(Math.abs(box.max.y-11)<1e-6)
    assert(box.min.x>=488-.003 && box.max.x<=512+.003)
    assert(box.min.z>=388-.003 && box.max.z<=412+.003)
    assert.equal(group.children.length,5,'no lawn/tree/extra lot geometry')
    const door=group.getObjectByName('front-door').getWorldPosition(new THREE.Vector3())
    const angle=THREE.MathUtils.degToRad(yaw)
    assert(Math.abs(door.x-(500-Math.sin(angle)*12.002))<1e-6)
    assert(Math.abs(door.z-(400-Math.cos(angle)*12.002))<1e-6)
    assert.equal(group.userData.footprintOnly,true)
  })
}
