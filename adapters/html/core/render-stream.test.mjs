// Node-side tests for the preview render stream.
//
// Regression covered (R6 P-4 / R7-3): the pending budget of 2 used to be a
// synchronous throw while `load()` starts every active chunk in one
// Promise.all, so any world with more than two chunks in view (b8 and the 1 km
// world both have four) failed with 'Render stream pending budget exceeded (2)'.
// The budget must be a CONCURRENCY limit that queues, and every request must
// eventually complete.
//
// Run: node --test render-stream.test.mjs
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { test } from "node:test";
import { createRenderStream, loadPreviewScene } from "./render-stream.js";

const REV = "rev-test";
const sha = (buf) => createHash("sha256").update(buf).digest("hex");

function makeWorld(n) {
  const chunks = [];
  const streamChunks = [];
  const payloads = new Map();
  for (let i = 0; i < n; i++) {
    const chunkX = i % 3;
    const chunkZ = Math.floor(i / 3);
    const file = `render-chunks/${chunkX}-${chunkZ}-${"0".repeat(64)}.json`;
    const data = {
      format: "glyphweave.render-chunk",
      version: 1,
      worldRevision: REV,
      sceneId: "scene-0",
      chunkX,
      chunkZ,
      patches: [{ id: `p${i}`, material: "access_pavement" }],
      plants: i === 0 ? [] : [{ id: `t${i}`, sourcePatchId: `p${i}` }],
    };
    const bytes = Buffer.from(JSON.stringify(data));
    const digest = sha(bytes);
    const located = `render-chunks/${chunkX}-${chunkZ}-${digest}.json`;
    chunks.push({ chunkX, chunkZ });
    streamChunks.push({ chunkX, chunkZ, bytes: bytes.length, patchCount: 1, plantCount: data.plants.length, sha256: digest, file: located });
    payloads.set(located, bytes);
  }
  return {
    scene: {
      sceneId: "scene-0",
      renderBase: "./",
      chunks,
      renderStream: { format: "glyphweave.render-directory", version: 1, worldRevision: REV, chunks: streamChunks, totalPatches: n, totalPlants: Math.max(0, n - 1) },
    },
    payloads,
  };
}

function fetcher(payloads, { corrupt = null } = {}) {
  return async (url) => {
    const key = String(url).replace(/^.*render-chunks\//, "render-chunks/");
    if (corrupt && key === corrupt) {
      const buf = Buffer.from("{}");
      return { ok: true, status: 200, arrayBuffer: async () => buf.buffer.slice(buf.byteOffset, buf.byteOffset + buf.byteLength) };
    }
    const buf = payloads.get(key);
    if (!buf) return { ok: false, status: 404 };
    return { ok: true, status: 200, arrayBuffer: async () => buf.buffer.slice(buf.byteOffset, buf.byteOffset + buf.byteLength) };
  };
}

test("more active chunks than the pending budget: queues instead of throwing", async () => {
  const { scene, payloads } = makeWorld(4);
  const stream = createRenderStream(scene, fetcher(payloads));
  const result = await stream.load(scene.chunks);
  const stats = stream.stats();
  assert.equal(stats.failures, 0, "no request may fail");
  assert.equal(stats.requests, 4, "all four chunks must be fetched");
  assert.equal(result.stats.loadedChunks.length, 4);
  assert.ok(stats.peakPending <= 2, `peakPending must stay inside the budget, got ${stats.peakPending}`);
  assert.equal(result.stats.loadedPatches, 4);
});

test("more than four active chunks still all complete", async () => {
  const { scene, payloads } = makeWorld(6);
  const stream = createRenderStream(scene, fetcher(payloads));
  await stream.load(scene.chunks);
  assert.equal(stream.stats().requests, 6);
  assert.ok(stream.stats().peakPending <= 2);
});

test("a corrupt chunk still fails closed", async () => {
  const { scene, payloads } = makeWorld(4);
  const bad = [...payloads.keys()][2];
  const stream = createRenderStream(scene, fetcher(payloads, { corrupt: bad }));
  await assert.rejects(() => stream.load(scene.chunks));
  assert.equal(stream.stats().failures, 1);
});

test("a legacy world without a render directory still loads its scene", async () => {
  const legacy = { sceneId: "scene-0" };
  const scene = await loadPreviewScene("scenes/scene-0/scene.json", REV, async (url) =>
    String(url).includes("preview-scene.json")
      ? { status: 404, ok: false, json: async () => ({}) }
      : { status: 200, ok: true, json: async () => legacy },
  );
  assert.equal(scene, legacy);
});
