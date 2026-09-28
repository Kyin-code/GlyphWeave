// Shared storage decoder: no rendering-only smoothing or competing height.
export function decodeHeightPayload(buffer,width,depth) {
  const view=new DataView(buffer), count=width*depth
  if(!Number.isInteger(width)||!Number.isInteger(depth)||width<1||depth<1||width>512||depth>512)throw new Error('Invalid height dimensions')
  let encoding='quarter-meter',offset=0,stride=2,divisor=4
  if(buffer.byteLength!==count*2) {
    if(buffer.byteLength!==16+count*4 || view.getUint32(0,false)!==0x4757484d || view.getUint32(4,true)!==1 || view.getUint32(8,true)!==width || view.getUint32(12,true)!==depth)throw new Error('Invalid millimetre height payload')
    encoding='millimeter';offset=16;stride=4;divisor=1000
  }
  return {encoding,readMetres(index){
    if(!Number.isInteger(index)||index<0||index>=count)throw new Error('Height sample outside payload')
    return (stride===4?view.getInt32(offset+index*stride,true):view.getInt16(index*stride,true))/divisor
  }}
}
