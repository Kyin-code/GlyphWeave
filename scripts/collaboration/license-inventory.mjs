#!/usr/bin/env node
// Generate a reproducible, lockfile-scoped license inventory for release review.
// This records facts and unresolved items; it is not a legal clearance opinion.
import {execFileSync} from 'node:child_process'
import {createHash} from 'node:crypto'
import {existsSync, readFileSync, readdirSync, statSync, writeFileSync} from 'node:fs'
import {dirname, join, resolve} from 'node:path'
import {fileURLToPath} from 'node:url'

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..')
const PNPM = process.platform === 'win32' ? 'pnpm.cmd' : 'pnpm'
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex')
const hashFile = path => sha256(readFileSync(path))
const runJson = (command, args) => JSON.parse(execFileSync(command, args, {
  cwd: ROOT,
  encoding: 'utf8',
  maxBuffer: 64 * 1024 * 1024,
  shell: process.platform === 'win32',
  stdio: ['ignore', 'pipe', 'pipe'],
}))

function licenseFiles(directory) {
  if (!directory || !existsSync(directory)) return []
  return readdirSync(directory, {withFileTypes: true})
    .filter(entry => entry.isFile() && /^(licen[cs]e|copying|notice|unlicense)(\.|$)/i.test(entry.name))
    .map(entry => {
      const path = join(directory, entry.name)
      return {name: entry.name, path, bytes: statSync(path).size, sha256: hashFile(path)}
    })
    .sort((a, b) => a.name.localeCompare(b.name))
}

function cargoInventory() {
  const lockPath = join(ROOT, 'bevy', 'Cargo.lock')
  const lockText = readFileSync(lockPath, 'utf8').replace(/\r\n/g, '\n')
  const lockPackages = []
  for (const match of lockText.matchAll(/^\[\[package\]\]\r?\nname = "([^"]+)"\r?\nversion = "([^"]+)"(?:\r?\nsource = "([^"]+)")?/gm)) {
    lockPackages.push({name: match[1], version: match[2], source: match[3] ?? null})
  }
  const metadata = runJson('cargo', ['metadata', '--manifest-path', 'bevy/Cargo.toml', '--locked', '--format-version', '1'])
  const metadataPackages = metadata.packages.map(pkg => {
    const packageDirectory = dirname(pkg.manifest_path)
    return {
      name: pkg.name,
      version: pkg.version,
      source: pkg.source,
      license: pkg.license,
      licenseFile: pkg.license_file,
      manifestPath: pkg.manifest_path,
      licenseFiles: licenseFiles(packageDirectory),
    }
  })
  const key = pkg => `${pkg.name}@${pkg.version}@${pkg.source ?? 'path'}`
  const metadataKeys = new Set(metadataPackages.map(key))
  const missingFromMetadata = lockPackages.filter(pkg => !metadataKeys.has(key(pkg)))
  return {
    lockfile: {path: lockPath, sha256: hashFile(lockPath), packageCount: lockPackages.length},
    metadataPackageCount: metadataPackages.length,
    missingFromMetadata,
    packages: metadataPackages.sort((a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version)),
  }
}

function parsePnpmPackageKeys(lockText) {
  const start = lockText.indexOf('\npackages:\n')
  const end = lockText.indexOf('\nsnapshots:\n', start + 1)
  if (start < 0 || end < 0) throw new Error('pnpm-lock.yaml packages/snapshots sections are missing')
  const section = lockText.slice(start + 1, end)
  const packages = []
  for (const line of section.split(/\r?\n/)) {
    const match = /^  '?((?:@[^']+\/)?[^'@][^':]*?)@([0-9][^':]*?)'?:$/.exec(line)
    if (match) packages.push({name: match[1], version: match[2]})
  }
  return [...new Map(packages.map(pkg => [`${pkg.name}@${pkg.version}`, pkg])).values()]
    .sort((a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version))
}

function pnpmInventory() {
  const lockPath = join(ROOT, 'pnpm-lock.yaml')
  const lockText = readFileSync(lockPath, 'utf8').replace(/\r\n/g, '\n')
  const lockPackages = parsePnpmPackageKeys(lockText)
  const pnpmStore = join(ROOT, 'node_modules', '.pnpm')
  const installedDirectories = (name, version) => {
    if (!existsSync(pnpmStore)) return []
    const encoded = name.replace('/', '+')
    return readdirSync(pnpmStore, {withFileTypes: true})
      .filter(entry => entry.isDirectory() && entry.name.startsWith(`${encoded}@${version}`))
      .map(entry => join(pnpmStore, entry.name))
  }
  const installedPackage = (name, version) => {
    for (const directory of installedDirectories(name, version)) {
      const candidate = join(directory, 'node_modules', ...name.split('/'))
      if (existsSync(join(candidate, 'package.json'))) return candidate
    }
    return null
  }
  const listed = new Map()
  const commandResults = {}
  for (const scope of ['prod', 'dev']) {
    const args = ['licenses', 'list', `--${scope}`, '--json']
    try {
      const data = runJson(PNPM, args)
      commandResults[scope] = {command: `pnpm ${args.join(' ')}`, status: 'pass'}
      for (const [license, entries] of Object.entries(data)) {
        for (const entry of entries) {
          for (let index = 0; index < entry.versions.length; index++) {
            const version = entry.versions[index]
            const packagePath = entry.paths?.[index] ?? entry.paths?.[0] ?? null
            const key = `${entry.name}@${version}`
            const current = listed.get(key) ?? {name: entry.name, version, licenses: [], paths: []}
            if (!current.licenses.includes(license)) current.licenses.push(license)
            if (packagePath && !current.paths.includes(packagePath)) current.paths.push(packagePath)
            listed.set(key, current)
          }
        }
      }
    } catch (error) {
      commandResults[scope] = {command: `pnpm ${args.join(' ')}`, status: 'fail', error: String(error.stderr || error.message || error)}
    }
  }
  const packages = lockPackages.map(pkg => {
    const listedPackage = listed.get(`${pkg.name}@${pkg.version}`)
    const fallbackPath = installedPackage(pkg.name, pkg.version)
    const paths = listedPackage?.paths?.length ? listedPackage.paths : fallbackPath ? [fallbackPath] : []
    const packageJson = paths
      .map(path => join(path, 'package.json'))
      .filter(path => existsSync(path))
      .map(path => JSON.parse(readFileSync(path, 'utf8')))
    const fallbackLicenses = packageJson.map(value => value.license).filter(Boolean)
    return {
      ...pkg,
      licenseExpressions: listedPackage?.licenses?.length ? listedPackage.licenses : fallbackLicenses,
      installedPaths: paths,
      licenseFiles: paths.flatMap(path => licenseFiles(path)),
      unresolvedStatus: listedPackage || fallbackLicenses.length > 0 ? null
        : installedDirectories(pkg.name, pkg.version).length > 0
          ? 'store-entry-present-without-installed-package-metadata'
          : 'not-installed-in-current-platform',
    }
  })
  const missingFromLicenseList = packages.filter(pkg => pkg.licenseExpressions.length === 0)
  return {
    lockfile: {path: lockPath, sha256: hashFile(lockPath), packageCount: lockPackages.length},
    listedPackageCount: listed.size,
    resolvedPackageCount: packages.filter(pkg => pkg.licenseExpressions.length > 0).length,
    commandResults,
    missingFromLicenseList,
    packages,
  }
}

function assetInventory() {
  const roots = [
    'adapters/html/vendor/three/LICENSE',
    'assets/third_party/quaternius/stylized-nature/License_Standard.txt',
    'assets/third_party/quaternius/buildings/NOTICE.txt',
  ]
  return roots.map(path => ({
    path: join(ROOT, path),
    bytes: statSync(join(ROOT, path)).size,
    sha256: hashFile(join(ROOT, path)),
  }))
}

const inventory = {
  format: 'glyphweave.release-license-inventory',
  version: 1,
  generatedAtUtc: new Date().toISOString(),
  scope: 'Lockfile packages, installed license metadata/text locations, vendored Three.js, and selected Quaternius assets. Not legal clearance.',
  cargo: cargoInventory(),
  pnpm: pnpmInventory(),
  assets: assetInventory(),
}

const outIndex = process.argv.indexOf('--out')
const output = JSON.stringify(inventory, null, 2) + '\n'
if (outIndex >= 0) {
  const outPath = process.argv[outIndex + 1]
  if (!outPath) throw new Error('--out requires a path')
  writeFileSync(resolve(outPath), output)
  console.log(JSON.stringify({
    format: inventory.format,
    cargoLocked: inventory.cargo.lockfile.packageCount,
    cargoMetadata: inventory.cargo.metadataPackageCount,
    pnpmLocked: inventory.pnpm.lockfile.packageCount,
    pnpmListed: inventory.pnpm.listedPackageCount,
    pnpmResolved: inventory.pnpm.resolvedPackageCount,
    pnpmMissing: inventory.pnpm.missingFromLicenseList.map(pkg => `${pkg.name}@${pkg.version}`),
    out: resolve(outPath),
  }, null, 2))
} else {
  process.stdout.write(output)
}
