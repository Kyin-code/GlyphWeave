// Rust supplies the longitudinal samples; this adapter neither samples a
// competing terrain surface nor flattens the road to entity.worldY.
export function gradedRoadGeometry(entity) {
  const p=entity.grounding?.roadProfile
  if(!p||p.version!==1||entity.kind!=='road')throw new Error('Missing Rust road profile')
  const dx=p.end[0]-p.start[0],dz=p.end[1]-p.start[1],length=Math.abs(dx)+Math.abs(dz)
  const width=entity.depthM
  if((dx===0)===(dz===0)||length<16||length>10000||!Number.isInteger(length)||p.elevationsMm.length!==length+1
    ||!Number.isInteger(width)||width<4||width>32||width%2!==0
    ||entity.widthM!==length||entity.scale!==1||entity.worldX*2!==p.start[0]+p.end[0]||entity.worldZ*2!==p.start[1]+p.end[1])throw new Error('Invalid Rust road profile domain')
  const positions=[],indices=[]
  for(let i=0;i<=length;i++) {
    const y=p.elevationsMm[i]
    if(!Number.isSafeInteger(y)||y< -32000||y>256000||(i>0&&Math.abs(y-p.elevationsMm[i-1])>80))throw new Error('Invalid Rust road profile height')
    for(const t of [-width/2,width/2])positions.push(p.start[0]+Math.sign(dx)*i+(dx===0?t:0)-entity.worldX,y/1000,p.start[1]+Math.sign(dz)*i+(dz===0?t:0)-entity.worldZ)
  }
  for(let i=0;i<length;i++) {
    const k=i*2,a=[k,k+1,k+2],b=[k+1,k+3,k+2]
    if(dx<0||dz>0){[a[1],a[2]]=[a[2],a[1]];[b[1],b[2]]=[b[2],b[1]]}
    indices.push(...a,...b)
  }
  return {positions,indices}
}
export function makeGradedRoad(entity,THREE) {
  const data=gradedRoadGeometry(entity),group=new THREE.Group()
  const makeBand=(leftScale,rightScale,lift,color)=>{
    const positions=[],indices=[]
    for(let i=0;i<data.positions.length;i+=6){
      const ax=data.positions[i],ay=data.positions[i+1],az=data.positions[i+2]
      const bx=data.positions[i+3],by=data.positions[i+4],bz=data.positions[i+5]
      const cx=(ax+bx)*.5,cz=(az+bz)*.5
      const left=[cx+(ax-cx)*leftScale,ay+lift,cz+(az-cz)*leftScale]
      const right=[cx+(bx-cx)*rightScale,by+lift,cz+(bz-cz)*rightScale]
      positions.push(...left,...right)
    }
    for(let i=0;i<data.indices.length/6;i++){
      const k=i*2,a=[k,k+1,k+2],b=[k+1,k+3,k+2]
      indices.push(...a,...b)
    }
    const geometry=new THREE.BufferGeometry()
    geometry.setAttribute('position',new THREE.Float32BufferAttribute(positions,3))
    geometry.setIndex(indices);geometry.computeVertexNormals()
    return new THREE.Mesh(geometry,new THREE.MeshStandardMaterial({color,roughness:.95,polygonOffset:true,polygonOffsetFactor:-1,polygonOffsetUnits:-1}))
  }
  const road=makeBand(-1,1,.02,'#3f4748')
  const walkLeft=makeBand(-1,-.76,.11,'#9c9486')
  const walkRight=makeBand(.76,1,.11,'#9c9486')
  const marking=makeBand(-.025,.025,.08,'#d9cc99')
  group.add(road,walkLeft,walkRight,marking)
  group.position.set(entity.worldX,0,entity.worldZ);group.name='graded-road:'+entity.entityId
  group.userData.sourceRoadId=entity.entityId;group.userData.profileVersion=1;group.userData.authoredWorldAxes=true
  group.traverse(child=>{if(child.isMesh)child.receiveShadow=true})
  return group
}
