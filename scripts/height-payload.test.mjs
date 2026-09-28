import {test} from 'node:test'
import assert from 'node:assert/strict'
import {decodeHeightPayload} from '../adapters/html/core/height.js'
import * as THREE from '../adapters/html/vendor/three/build/three.module.js'
import {makeMCTerrain} from '../adapters/html/render/terrain.js'
function mm(values,w,d){const b=new ArrayBuffer(16+values.length*4),v=new DataView(b);v.setUint32(0,0x4757484d,false);v.setUint32(4,1,true);v.setUint32(8,w,true);v.setUint32(12,d,true);values.forEach((x,i)=>v.setInt32(16+i*4,x,true));return b}
test('signed units and small slopes are identical at decoder and terrain mesh',()=>{
 const b=mm([-1234,-1194,-1154,-1114],2,2),p=decodeHeightPayload(b,2,2)
 assert.equal(p.encoding,'millimeter');assert.equal(p.readMetres(0),-1.234);assert.equal(p.readMetres(1),-1.194)
 const geom=makeMCTerrain(p,new Uint8Array(4),null,2,2,1,THREE,0)
 const pos=geom.getAttribute('position');let precise=false
 for(let i=0;i<pos.count;i++){const h=pos.getY(i);assert(h>=-1.235&&h<=-1.113);if(Math.abs(h*4-Math.round(h*4))>.01)precise=true}
 assert(precise,'renderer quantized mm heights back to quarter metres')
})
test('legacy compatibility and malformed header/index failure',()=>{
 const b=new ArrayBuffer(4),v=new DataView(b);v.setInt16(0,-5,true);v.setInt16(2,3,true)
 assert.equal(decodeHeightPayload(b,2,1).readMetres(0),-1.25)
 const good=mm([1,2],2,1)
 for(const at of [0,4,8,12]){const bad=good.slice(0);new Uint8Array(bad)[at]^=1;assert.throws(()=>decodeHeightPayload(bad,2,1))}
 assert.throws(()=>decodeHeightPayload(good.slice(0,-1),2,1));assert.throws(()=>decodeHeightPayload(good,1,2));assert.throws(()=>decodeHeightPayload(good,2,1).readMetres(2))
})
test('metre samples are not stretched over chunk width or partial LOD cells',()=>{
 const w=5,d=3,p=decodeHeightPayload(mm(Array.from({length:w*d},(_,i)=>i*10),w,d),w,d)
 for(const step of [1,2,4]){
  const g=makeMCTerrain(p,new Uint8Array(w*d),null,w,d,step,THREE)
  const a=g.getAttribute('position')
  for(let i=0;i<a.count;i++){
   const x=a.getX(i)+w/2,z=a.getZ(i)+d/2
   assert.equal(x,Math.round(x));assert.equal(z,Math.round(z))
   assert(Math.abs(a.getY(i)-p.readMetres(Math.min(z,d-1)*w+Math.min(x,w-1)))<1e-6)
  }
 }
})
