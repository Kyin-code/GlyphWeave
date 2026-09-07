// Rust supplies the longitudinal samples; this adapter neither samples a
// competing terrain surface nor flattens the road to entity.worldY.
export function gradedRoadGeometry(entity) {
  const p=entity.grounding?.roadProfile
  if(!p||p.version!==1||entity.kind!=='road')throw new Error('Missing Rust road profile')
  const dx=p.end[0]-p.start[0],dz=p.end[1]-p.start[1],length=Math.abs(dx)+Math.abs(dz)
  if((dx===0)===(dz===0)||length<16||length>10000||!Number.isInteger(length)||p.elevationsMm.length!==length+1
    ||entity.widthM!==length||entity.depthM!==8||entity.scale!==1||entity.worldX*2!==p.start[0]+p.end[0]||entity.worldZ*2!==p.start[1]+p.end[1])throw new Error('Invalid Rust road profile domain')
  const positions=[],indices=[]
  for(let i=0;i<=length;i++) {
    const y=p.elevationsMm[i]
    if(!Number.isSafeInteger(y)||y< -32000||y>256000||(i>0&&Math.abs(y-p.elevationsMm[i-1])>80))throw new Error('Invalid Rust road profile height')
    for(const t of [-4,4])positions.push(p.start[0]+Math.sign(dx)*i+(dx===0?t:0)-entity.worldX,y/1000,p.start[1]+Math.sign(dz)*i+(dz===0?t:0)-entity.worldZ)
  }
  for(let i=0;i<length;i++) {
    const k=i*2,a=[k,k+1,k+2],b=[k+1,k+3,k+2]
    if(dx<0||dz>0){[a[1],a[2]]=[a[2],a[1]];[b[1],b[2]]=[b[2],b[1]]}
    indices.push(...a,...b)
  }
  return {positions,indices}
}
export function makeGradedRoad(entity,THREE) {
  const data=gradedRoadGeometry(entity),geometry=new THREE.BufferGeometry()
  geometry.setAttribute('position',new THREE.Float32BufferAttribute(data.positions,3));geometry.setIndex(data.indices);geometry.computeVertexNormals()
  const mesh=new THREE.Mesh(geometry,new THREE.MeshStandardMaterial({color:'#4c514e',roughness:.95,polygonOffset:true,polygonOffsetFactor:-1,polygonOffsetUnits:-1}))
  mesh.position.set(entity.worldX,0,entity.worldZ);mesh.name='graded-road:'+entity.entityId;mesh.receiveShadow=true
  mesh.userData.sourceRoadId=entity.entityId;mesh.userData.profileVersion=1;mesh.userData.authoredWorldAxes=true
  return mesh
}
