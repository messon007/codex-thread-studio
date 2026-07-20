import fs from 'node:fs'
import path from 'node:path'
import process from 'node:process'
import { fileURLToPath } from 'node:url'

const root = process.env.VERSION_PROJECT_ROOT
  ? path.resolve(process.env.VERSION_PROJECT_ROOT)
  : path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const cargoPath = path.join(root, 'src-tauri', 'Cargo.toml')
const lockPath = path.join(root, 'Cargo.lock')
const tauriPath = path.join(root, 'src-tauri', 'tauri.conf.json')
const changelogPath = path.join(root, 'CHANGELOG.md')
const semverPattern = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/u

function fail(message) {
  console.error(`version: ${message}`)
  process.exit(1)
}

function read(file) {
  return fs.readFileSync(file, 'utf8')
}

function cargoVersion(source = read(cargoPath)) {
  const match = source.match(/^\[package\][\s\S]*?^version\s*=\s*"([^"]+)"/m)
  if (!match) fail('src-tauri/Cargo.toml does not contain [package].version')
  return match[1]
}

function lockedVersion(source = read(lockPath)) {
  const match = source.match(/\[\[package\]\]\nname = "codex-thread-studio"\nversion = "([^"]+)"/u)
  if (!match) fail('Cargo.lock does not contain the codex-thread-studio package')
  return match[1]
}

function requestedTag() {
  const index = process.argv.indexOf('--tag')
  return index >= 0 ? process.argv[index + 1] : process.env.RELEASE_TAG
}

function check() {
  const version = cargoVersion()
  if (!semverPattern.test(version)) fail(`Cargo version is not valid SemVer: ${version}`)
  if (lockedVersion() !== version) fail(`Cargo.lock does not match Cargo.toml (${lockedVersion()} != ${version})`)

  const tauriConfig = JSON.parse(read(tauriPath))
  if (Object.hasOwn(tauriConfig, 'version')) {
    fail('tauri.conf.json must omit version so Tauri inherits Cargo.toml')
  }

  const changelog = read(changelogPath)
  if (!new RegExp(`^## \\[${version.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}\\](?:\\s|$)`, 'm').test(changelog)) {
    fail(`CHANGELOG.md has no ${version} release section`)
  }
  if (!changelog.includes(`[Unreleased]: https://github.com/messon007/codex-thread-studio/compare/v${version}...HEAD`)) {
    fail(`CHANGELOG.md Unreleased link does not start at v${version}`)
  }
  if (!changelog.includes(`[${version}]:`)) fail(`CHANGELOG.md has no link for ${version}`)

  const tag = requestedTag()
  if (tag && tag !== `v${version}`) fail(`tag ${tag} does not match Cargo version v${version}`)
  console.log(`Codex Thread Studio v${version}${tag ? ` matches ${tag}` : ''}`)
}

function bump(nextVersion) {
  if (!nextVersion || !semverPattern.test(nextVersion)) fail('usage: npm run version:bump -- <semver>')
  const currentVersion = cargoVersion()
  if (nextVersion === currentVersion) fail(`version is already ${nextVersion}`)

  const changelog = read(changelogPath)
  const unreleased = changelog.match(/(## \[Unreleased\]\s*\n)([\s\S]*?)(?=\n## \[)/u)
  if (!unreleased) fail('CHANGELOG.md must contain Unreleased before the latest release')
  const notes = unreleased[2].trim()
  if (!notes) fail('add release notes under CHANGELOG.md Unreleased before bumping')

  const cargo = read(cargoPath).replace(
    /(^\[package\][\s\S]*?^version\s*=\s*")[^"]+("\s*$)/m,
    `$1${nextVersion}$2`,
  )
  const lock = read(lockPath).replace(
    /(\[\[package\]\]\nname = "codex-thread-studio"\nversion = ")[^"]+("\n)/u,
    `$1${nextVersion}$2`,
  )
  const date = new Date().toISOString().slice(0, 10)
  let nextChangelog = changelog.replace(
    unreleased[0],
    `## [Unreleased]\n\n## [${nextVersion}] - ${date}\n\n${notes}\n`,
  )
  const link = `[Unreleased]: https://github.com/messon007/codex-thread-studio/compare/v${currentVersion}...HEAD`
  if (!nextChangelog.includes(link)) fail(`CHANGELOG.md Unreleased link does not start at v${currentVersion}`)
  nextChangelog = nextChangelog.replace(
    link,
    `[Unreleased]: https://github.com/messon007/codex-thread-studio/compare/v${nextVersion}...HEAD\n[${nextVersion}]: https://github.com/messon007/codex-thread-studio/compare/v${currentVersion}...v${nextVersion}`,
  )

  fs.writeFileSync(cargoPath, cargo)
  fs.writeFileSync(lockPath, lock)
  fs.writeFileSync(changelogPath, nextChangelog)
  console.log(`Bumped Codex Thread Studio ${currentVersion} -> ${nextVersion}`)
}

const command = process.argv[2] || 'check'
if (command === 'check') check()
else if (command === 'bump') bump(process.argv[3])
else fail(`unknown command: ${command}`)
