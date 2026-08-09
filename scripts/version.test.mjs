import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'

const script = path.join(path.dirname(fileURLToPath(import.meta.url)), 'version.mjs')

function fixture(lineEnding = '\n') {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'codex-thread-studio-version-'))
  fs.mkdirSync(path.join(root, 'src-tauri'), { recursive: true })
  fs.writeFileSync(
    path.join(root, 'src-tauri', 'Cargo.toml'),
    ['[package]', 'name = "codex-thread-studio"', 'version = "0.2.0"', ''].join(lineEnding),
  )
  fs.writeFileSync(
    path.join(root, 'Cargo.lock'),
    ['[[package]]', 'name = "codex-thread-studio"', 'version = "0.2.0"', ''].join(lineEnding),
  )
  fs.writeFileSync(path.join(root, 'src-tauri', 'tauri.conf.json'), '{"productName":"Codex Thread Studio"}\n')
  fs.writeFileSync(path.join(root, 'CHANGELOG.md'), '# Changelog\n\n## [Unreleased]\n\n- Next feature.\n\n## [0.2.0] - 2026-07-20\n\n- Current release.\n\n[Unreleased]: https://github.com/messon007/codex-thread-studio/compare/v0.2.0...HEAD\n[0.2.0]: https://github.com/messon007/codex-thread-studio/releases/tag/v0.2.0\n')
  return root
}

function version(root, ...args) {
  return spawnSync(process.execPath, [script, ...args], {
    encoding: 'utf8',
    env: { ...process.env, VERSION_PROJECT_ROOT: root },
  })
}

test('checks the single version source and an optional matching tag', (context) => {
  const root = fixture()
  context.after(() => fs.rmSync(root, { recursive: true, force: true }))
  assert.equal(version(root, 'check', '--tag', 'v0.2.0').status, 0)
  const mismatch = version(root, 'check', '--tag', 'v0.3.0')
  assert.equal(mismatch.status, 1)
  assert.match(mismatch.stderr, /does not match Cargo version/u)
})

test('checks and updates Cargo metadata with Windows line endings', (context) => {
  const root = fixture('\r\n')
  context.after(() => fs.rmSync(root, { recursive: true, force: true }))
  assert.equal(version(root, 'check').status, 0)
  assert.equal(version(root, 'bump', '0.3.0').status, 0)
  assert.match(
    fs.readFileSync(path.join(root, 'Cargo.lock'), 'utf8'),
    /name = "codex-thread-studio"\r\nversion = "0\.3\.0"\r\n/u,
  )
})

test('moves Unreleased notes and synchronizes generated Cargo metadata', (context) => {
  const root = fixture()
  context.after(() => fs.rmSync(root, { recursive: true, force: true }))
  assert.equal(version(root, 'bump', '0.3.0').status, 0)
  assert.match(fs.readFileSync(path.join(root, 'src-tauri', 'Cargo.toml'), 'utf8'), /version = "0\.3\.0"/u)
  assert.match(fs.readFileSync(path.join(root, 'Cargo.lock'), 'utf8'), /version = "0\.3\.0"/u)
  const changelog = fs.readFileSync(path.join(root, 'CHANGELOG.md'), 'utf8')
  assert.match(changelog, /## \[Unreleased\]\n\n## \[0\.3\.0\] - \d{4}-\d{2}-\d{2}\n\n- Next feature\./u)
  assert.match(changelog, /\[Unreleased\]: .*\/compare\/v0\.3\.0\.\.\.HEAD/u)
  assert.match(changelog, /\[0\.3\.0\]: .*\/compare\/v0\.2\.0\.\.\.v0\.3\.0/u)
  assert.equal(version(root, 'check').status, 0)
})
