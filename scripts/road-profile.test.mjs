import {test} from 'node:test'
import assert from 'node:assert/strict'
import {gradedRoadGeometry} from '../adapters/html/render/road-profile.js'
const entity=(start,end)=>({entityId:'road',kind:'road',worldX:(start[0]+end[0])/2,worldZ:(start[1]+end[1])/2,widthM:64,depthM:8,scale:1,grounding:{roadProfile:{version:1,start,end,elevationsMm:Array.from({length:65},(_,i)=>1000+i*40)}}})
test('graded roads preserve each Rust station and upward winding for four directions',()=>{
  for(const [start,end] of [[[-1000,50],[-936,50]],[[-936,50],[-1000,50]],[[-1000,50],[-1000,114]],[[-1000,114],[-1000,50]]]) {
    const e=entity(start,end),g=gradedRoadGeometry(e)
    assert.equal(g.positions.length,130*3);assert.equal(g.indices.length,128*3)
    for(let i=0;i<65;i++){assert.equal(g.positions[i*6+1],(1000+i*40)/1000);assert.equal(g.positions[i*6+4],(1000+i*40)/1000)}
    for(let i=0;i<g.indices.length;i+=3){const [a,b,c]=g.indices.slice(i,i+3).map(k=>g.positions.slice(k*3,k*3+3));assert((b[2]-a[2])*(c[0]-a[0])-(b[0]-a[0])*(c[2]-a[2])>0)}
  }
})
test('invalid or unsupported authored road profiles fail instead of flattening',()=>{
  for(const mutate of [e=>e.grounding.roadProfile.elevationsMm.pop(),e=>e.grounding.roadProfile.elevationsMm[1]=99999,e=>e.scale=2,e=>e.kind='building',e=>e.grounding.roadProfile.version=2]){const e=entity([0,0],[64,0]);mutate(e);assert.throws(()=>gradedRoadGeometry(e))}
})
