import {createHash} from 'node:crypto'
import {open, realpath, stat} from 'node:fs/promises'
import {isAbsolute, relative, resolve, sep} from 'node:path'

export const MAX_AUDIT_INPUT_BYTES = 64 * 1024 * 1024

export class AuditInputError extends Error {
  constructor(code, reason) {
    super(reason)
    this.code = code
  }
}

const need = (ok, code, reason) => {
  if (!ok) throw new AuditInputError(code, reason)
}

const inside = (child, parent) => {
  const rel = relative(parent, child)
  return rel === '' || (!isAbsolute(rel) && rel !== '..' && !rel.startsWith('..' + sep))
}

const digest = bytes => createHash('sha256').update(bytes).digest('hex')

export async function verifyAuditInputBinding(auditDir, parity, sceneIndex, {parse = false} = {}) {
  need(parity && parity.format === 'glyphweave.render-parity' && parity.version === 1,
    'audit.parity', 'parity format/version is not glyphweave.render-parity/v1')
  need(parity.status === 'pass', 'audit.parity-status', 'parity must report pass')
  need(Array.isArray(parity.scenes) && parity.scenes[sceneIndex],
    'audit.scene', `parity scene ${sceneIndex} is missing`)
  const scene = parity.scenes[sceneIndex]
  const binding = scene.auditInput
  need(binding && typeof binding.path === 'string' && binding.path.length > 0
    && Number.isSafeInteger(binding.bytes) && binding.bytes > 0
    && typeof binding.sha256 === 'string' && /^[0-9a-f]{64}$/i.test(binding.sha256),
    'audit.binding', `scene ${scene.sceneId} auditInput {path,bytes,sha256} is required`)
  const root = await realpath(resolve(auditDir))
  const candidate = resolve(root, binding.path)
  let filePath
  try {
    filePath = await realpath(candidate)
  } catch {
    throw new AuditInputError('audit.missing', `auditInput file is missing: ${binding.path}`)
  }
  need(inside(filePath, root), 'audit.escape', `auditInput path escapes parity directory: ${binding.path}`)
  const info = await stat(filePath)
  need(info.isFile(), 'audit.file', `auditInput is not a regular file: ${binding.path}`)
  need(info.size === binding.bytes, 'audit.bytes', `auditInput bytes mismatch: expected ${binding.bytes}, got ${info.size}`)
  need(info.size <= MAX_AUDIT_INPUT_BYTES, 'audit.budget', `auditInput exceeds ${MAX_AUDIT_INPUT_BYTES} bytes`)
  const handle = await open(filePath, 'r')
  let bytes
  try {
    bytes = Buffer.alloc(info.size)
    let offset = 0
    while (offset < bytes.length) {
      const read = await handle.read(bytes, offset, bytes.length - offset, offset)
      need(read.bytesRead > 0, 'audit.read', `auditInput truncated while reading ${binding.path}`)
      offset += read.bytesRead
    }
  } finally {
    await handle.close()
  }
  const sha256 = digest(bytes)
  need(sha256 === binding.sha256.toLowerCase(), 'audit.hash',
    `auditInput sha256 mismatch: expected ${binding.sha256}, got ${sha256}`)
  if (!parse) return {path: filePath, bytes: bytes.length, sha256}
  let data
  try {
    data = JSON.parse(bytes.toString('utf8'))
  } catch (error) {
    throw new AuditInputError('audit.json', `auditInput is not valid JSON: ${String(error.message || error)}`)
  }
  need(data && data.sceneId === scene.sceneId, 'audit.scene-id',
    `auditInput sceneId mismatch: expected ${scene.sceneId}, got ${data?.sceneId}`)
  need(data.worldRevision === parity.worldRevision, 'audit.revision',
    `auditInput worldRevision mismatch: expected ${parity.worldRevision}, got ${data.worldRevision}`)
  return {data, path: filePath, bytes: bytes.length, sha256}
}

export async function loadAuditInput(auditDir, parity, sceneIndex) {
  return (await verifyAuditInputBinding(auditDir, parity, sceneIndex, {parse: true})).data
}
