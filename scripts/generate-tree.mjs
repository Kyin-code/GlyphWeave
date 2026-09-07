#!/usr/bin/env node
/**
 * generate-tree.mjs
 *
 * Walks the src/ directory and outputs a formatted directory tree
 * matching the style used in README.md and AGENTS.md.
 *
 * Usage:
 *   node scripts/generate-tree.mjs              # print tree (with connectors)
 *   node scripts/generate-tree.mjs --raw        # print flat file list
 *   node scripts/generate-tree.mjs --check      # exit 1 if tree structure differs from docs
 */

import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const ROOT = path.resolve(__dirname, '..')
const SRC = path.join(ROOT, 'src')

// Directories to exclude from the tree
const EXCLUDE_DIRS = new Set()

// --- File walker -----------------------------------------------------------------

function walk(dir, indent = '') {
  let entries
  try {
    entries = fs.readdirSync(dir, { withFileTypes: true })
  } catch {
    return []
  }

  const files = []
  const subdirs = []

  for (const entry of entries) {
    if (entry.name.startsWith('.')) continue
    if (entry.isDirectory()) {
      if (EXCLUDE_DIRS.has(entry.name)) continue
      subdirs.push(entry.name)
    } else {
      files.push(entry.name)
    }
  }

  // Sort: dirs first, then files; alphabetical
  const sorted = [
    ...subdirs.sort().map(d => ({ name: d, isDir: true })),
    ...files.sort().map(f => ({ name: f, isDir: false })),
  ]

  const result = []
  for (let i = 0; i < sorted.length; i++) {
    const { name, isDir } = sorted[i]
    const isLast = i === sorted.length - 1
    const connector = isLast ? '└── ' : '├── '

    if (isDir) {
      result.push({ text: `${indent}${connector}${name}/`, indent })
      const childIndent = indent + (isLast ? '    ' : '│   ')
      result.push(...walk(path.join(dir, name), childIndent))
    } else {
      result.push({ text: `${indent}${connector}${name}`, indent })
    }
  }
  return result
}

// --- Generate tree ---------------------------------------------------------------

function generateTree() {
  const entries = walk(SRC)
  return `src/\n${entries.map(e => e.text).join('\n')}`
}

// --- Extract & strip tree from markdown ------------------------------------------

/**
 * Find the first fenced code block that looks like a directory tree
 * (starts with `src/`).
 */
function findTreeBlock(filePath) {
  const content = fs.readFileSync(filePath, 'utf-8').replace(/\r\n/g, '\n')
  const regex = /```\n(src\/[\s\S]*?)```/
  const match = content.match(regex)
  if (!match) return null
  return { raw: match[1].trimEnd(), fullMatch: match[0], index: match.index }
}

/** Strip comments (everything from # to end-of-line) from a tree string */
function stripComments(tree) {
  return tree.replace(/\s*#.*$/gm, '').trimEnd()
}

/**
 * Parse a tree string into a sorted list of canonical directory paths.
 * e.g. "canvas/" under "├── components/" becomes "components/canvas/"
 */
function parseDirs(tree) {
  const raw = stripComments(tree)
  const lines = raw.split('\n').filter(Boolean)

  // Track indentation → current path prefix
  const stack = [{ indent: -1, path: [] }]
  const dirs = []

  for (const line of lines) {
    const trimmed = line.replace(/^[│├└─\s]+/, '').trim()
    if (!trimmed) continue

    // Calculate indent level from leading tree connectors/spaces
    const leading = line.match(/^[│├└─\s]*/)[0]
    const indent = leading.replace(/[^│\s]/g, '').replace(/\s\s/g, '│').length

    // Pop stack back to correct level
    while (stack.length > 1 && stack[stack.length - 1].indent >= indent) {
      stack.pop()
    }

    if (trimmed.endsWith('/')) {
      const dirName = trimmed.replace(/\/$/, '')
      const path = [...stack[stack.length - 1].path, dirName]
      dirs.push(path.join('/') + '/')
      stack.push({ indent, path })
    }
  }

  return dirs.sort()
}

/** Get actual directory structure from filesystem (with src/ prefix) */
function getActualDirs(root) {
  const dirs = ['src/']
  function walkDir(dir, relPath) {
    let entries
    try { entries = fs.readdirSync(dir, { withFileTypes: true }) } catch { return }
    for (const e of entries) {
      if (e.name.startsWith('.')) continue
      if (e.isDirectory() && !EXCLUDE_DIRS.has(e.name)) {
        const p = relPath ? `${relPath}/${e.name}/` : `src/${e.name}/`
        dirs.push(p)
        walkDir(path.join(dir, e.name), relPath ? `${relPath}/${e.name}` : `src/${e.name}`)
      }
    }
  }
  walkDir(root, '')
  return dirs.sort()
}

/** Exit code comparison: 0 = match, 1 = mismatch */
function checkTree() {
  const actualDirs = getActualDirs(SRC)
  const actualSig = actualDirs.join('\n')
  let upToDate = true

  for (const file of ['AGENTS.md']) {
    const fpath = path.join(ROOT, file)
    const block = findTreeBlock(fpath)
    if (!block) {
      console.log(`[WARN] No tree block found in ${file}`)
      upToDate = false
      continue
    }
    const docDirs = parseDirs(block.raw)
    const docSig = docDirs.join('\n')
    if (docSig !== actualSig) {
      console.log(`[FAIL] ${file} tree is OUTDATED`)
      // Show diff
      const added = actualDirs.filter(d => !docDirs.includes(d))
      const removed = docDirs.filter(d => !actualDirs.includes(d))
      if (added.length) console.log(`   + Added dirs: ${added.join(', ')}`)
      if (removed.length) console.log(`   - Removed dirs: ${removed.join(', ')}`)
      upToDate = false
    } else {
      console.log(`[PASS] ${file} tree is up to date`)
    }
  }

  if (!upToDate) {
    console.log('\nRun `node scripts/generate-tree.mjs` to see the current full tree.')
  }

  return upToDate
}

// --- Update tree in file ---------------------------------------------------------

function updateFileTree(filePath, newTree) {
  const block = findTreeBlock(filePath)
  if (!block) {
    console.error(`[ERROR] Could not find tree block in ${filePath}`)
    return false
  }

  const content = fs.readFileSync(filePath, 'utf-8')
  const updated = content.slice(0, block.index) +
    '```\n' + newTree + '\n```' +
    content.slice(block.index + block.fullMatch.length)

  fs.writeFileSync(filePath, updated, 'utf-8')
  return true
}

// --- Main ------------------------------------------------------------------------

function main() {
  const args = process.argv.slice(2)
  const mode = args.includes('--check') ? 'check'
    : args.includes('--raw') ? 'raw'
    : 'tree'

  if (mode === 'raw') {
    const files = fs.readdirSync(SRC, { recursive: true, withFileTypes: true })
      .filter(d => d.isFile() && !d.name.startsWith('.'))
      .map(d => path.relative(SRC, path.join(d.parentPath, d.name)))
      .sort()
    console.log(files.join('\n'))
    return
  }

  if (mode === 'check') {
    const upToDate = checkTree()
    process.exit(upToDate ? 0 : 1)
  }

  const tree = generateTree()

  // Default: print tree
  console.log(tree)
}

main()
