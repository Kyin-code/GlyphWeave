import { test } from 'node:test'
import assert from 'node:assert/strict'
import * as THREE from '../adapters/html/vendor/three/build/three.module.js'
import { makePlannedServiceBuilding } from '../adapters/html/render/buildings.js'
for (const [kind, widthM, depthM, heightM] of [['convenience_store',12,10,4],['school',40,30,10],['church',20,28,10],['town_hall',48,32,12],['market',36,30,6]]) {
  for (const yaw of [0,90,180,270]) test(`exact planned ${kind} dimensions and door at yaw ${yaw}`, () => {
    const entity={entityId:'service.'+kind,kind,widthM,depthM,heightM,worldX:-100,worldZ:220,worldY:5}
    const group=makePlannedServiceBuilding(entity,THREE)
    group.rotation.y=THREE.MathUtils.degToRad(yaw);group.updateMatrixWorld(true)
    const box=new THREE.Box3().setFromObject(group)
    const hw=(yaw%180?depthM:widthM)/2,hd=(yaw%180?widthM:depthM)/2
    assert(box.min.x>=-100-hw-.003 && box.max.x<=-100+hw+.003)
    assert(box.min.z>=220-hd-.003 && box.max.z<=220+hd+.003)
    assert(Math.abs(box.min.y-5)<1e-6 && Math.abs(box.max.y-5-heightM)<1e-6)
    const door=group.getObjectByName('front-door').getWorldPosition(new THREE.Vector3())
    const angle=THREE.MathUtils.degToRad(yaw)
    assert(Math.abs(door.x-(-100-Math.sin(angle)*(depthM/2+.002)))<1e-6)
    assert(Math.abs(door.z-(220-Math.cos(angle)*(depthM/2+.002)))<1e-6)
    assert.equal(group.children.length,5,'no invented campus, parking or garden')
    assert.equal(group.userData.programKind,kind)
  })
}
