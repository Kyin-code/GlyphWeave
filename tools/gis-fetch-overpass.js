// Fetch Shenzhen Bay GIS layers from Overpass API, serial with delays to
// avoid rate limiting. Saves each layer to artifacts/gis/.
const fs = require('fs');
const path = require('path');
const OUT = 'G:/difyllmwiki/artifacts/gis';
const BBOX = '22.485,113.930,22.515,113.970';

const queries = [
  { name: 'buildings', q: `[out:json][timeout:60];(way["building"](${BBOX}););out body;>;out skel qt;` },
  { name: 'roads', q: `[out:json][timeout:60];(way["highway"](${BBOX}););out body;>;out skel qt;` },
  { name: 'water', q: `[out:json][timeout:60];(way["natural"="water"](${BBOX});way["waterway"](${BBOX});relation["natural"="water"](${BBOX}););out body;>;out skel qt;` },
  { name: 'green', q: `[out:json][timeout:60];(way["landuse"="forest"](${BBOX});way["leisure"="park"](${BBOX});way["landuse"="grass"](${BBOX}););out body;>;out skel qt;` },
  { name: 'bridge', q: `[out:json][timeout:60];(way["bridge"="yes"](${BBOX}););out body;>;out skel qt;` },
];

async function fetchLayer(layer) {
  for (let attempt = 0; attempt < 4; attempt++) {
    try {
      const r = await fetch('https://overpass-api.de/api/interpreter', {
        method: 'POST',
        headers: { 'Content-Type': 'application/x-www-form-urlencoded', 'User-Agent': 'GlyphWeave-GIS/1.0' },
        body: 'data=' + encodeURIComponent(layer.q)
      });
      const t = await r.text();
      let j;
      try { j = JSON.parse(t); } catch(e) { throw new Error('non-json: ' + t.substring(0, 80)); }
      const ways = (j.elements||[]).filter(e => e.type === 'way').length;
      fs.writeFileSync(path.join(OUT, `szbay-${layer.name}.json`), JSON.stringify(j));
      console.log(`${layer.name}: OK elements=${j.elements.length} ways=${ways}`);
      return;
    } catch(e) {
      console.log(`${layer.name}: attempt ${attempt+1} failed: ${e.message}`);
      await new Promise(r => setTimeout(r, 8000 * (attempt + 1)));
    }
  }
  console.log(`${layer.name}: GAVE UP`);
}

(async () => {
  fs.mkdirSync(OUT, { recursive: true });
  for (const layer of queries) {
    await fetchLayer(layer);
    await new Promise(r => setTimeout(r, 4000));
  }
  console.log('all done');
})();
