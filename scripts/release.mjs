#!/usr/bin/env node
// CHANGELOG.md and the version, from the Conventional Commits (git-cliff,
// cliff.toml). See docs/workflow.md → «Выпуск».
//
//   node scripts/release.mjs changelog        (mise run changelog)
//     Refresh the commit list in «## [Unreleased]» — between the
//     <!-- commits:start/end --> markers; text written above them stays.
//
//   node scripts/release.mjs bump [auto|patch|minor|major|X.Y.Z]   (mise run bump)
//     The next version (auto: from the commits since the last release —
//     feat → minor, everything else → patch), written into Cargo.toml,
//     Cargo.lock, crates/app/tauri.conf.json and the Flatpak metainfo;
//     «## [Unreleased]» becomes «## [X.Y.Z] — date» and a new empty one opens.
//
// Needs a git checkout with the history (the colocated jj main checkout —
// a jj side workspace has no .git).

import { execFileSync } from 'node:child_process'
import { readFileSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const CHANGELOG = join(root, 'CHANGELOG.md')
const START = '<!-- commits:start -->'
const END = '<!-- commits:end -->'
const RELEASE_TAG = /^v(\d+)\.(\d+)\.(\d+)$/

const run = (command, args) => execFileSync(command, args, { cwd: root, encoding: 'utf8' }).trim()
const fail = (message) => {
  console.error(`release: ${message}`)
  process.exit(1)
}

function requireGit() {
  try {
    run('git', ['rev-parse', '--git-dir'])
  } catch {
    fail('needs a git checkout with the history — run it in the main (colocated) checkout, not a jj side workspace')
  }
}

const cliff = (args) =>
  execFileSync('pnpm', ['exec', 'git-cliff', '--config', 'cliff.toml', ...args], {
    cwd: root,
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'ignore'],
  }).trim()

const parse = (version) => version.split('.').map(Number)
const compare = (left, right) => {
  const [a, b] = [parse(left), parse(right)]
  for (let index = 0; index < 3; index += 1) if (a[index] !== b[index]) return a[index] - b[index]
  return 0
}

function currentVersion() {
  const config = JSON.parse(readFileSync(join(root, 'crates/app/tauri.conf.json'), 'utf8'))
  return config.version
}

function lastRelease() {
  const tags = run('git', ['tag', '--list', 'v*']).split('\n').filter((tag) => RELEASE_TAG.test(tag))
  return tags.map((tag) => tag.slice(1)).sort(compare).at(-1) ?? null
}

/** The «## [Unreleased]» section: [start, end) offsets into the changelog. */
function unreleased(text) {
  const start = text.indexOf('## [Unreleased]')
  if (start < 0) fail('CHANGELOG.md has no «## [Unreleased]» section')
  const next = text.indexOf('\n## [', start + 1)
  return { start, end: next < 0 ? text.length : next + 1 }
}

function commitList() {
  const body = cliff(['--unreleased', '--strip', 'all'])
  return body || '_Коммитов с прошлого релиза нет._'
}

/** The changelog with the commit block of [Unreleased] replaced by `list`. */
function withCommits(text, list) {
  const { start, end } = unreleased(text)
  let section = text.slice(start, end).trimEnd()
  const block = `${START}\n\n${list}\n\n${END}`
  if (section.includes(START) && section.includes(END)) {
    section = section.slice(0, section.indexOf(START)) + block + section.slice(section.indexOf(END) + END.length)
  } else {
    section = `${section}\n\n### Коммиты\n\n${block}`
  }
  return `${text.slice(0, start)}${section}\n\n${text.slice(end)}`.replace(/\n{3,}/g, '\n\n')
}

function changelog() {
  requireGit()
  const text = withCommits(readFileSync(CHANGELOG, 'utf8'), commitList())
  writeFileSync(CHANGELOG, text)
  console.log('CHANGELOG.md: [Unreleased] commit list refreshed')
}

function nextVersion(level, current) {
  if (/^\d+\.\d+\.\d+$/.test(level)) return level
  const released = lastRelease()
  if (level === 'auto') {
    // The first release ships what the files say.
    if (!released) return current
    const bumped = cliff(['--bumped-version']).replace(/^v/, '')
    return compare(current, bumped) > 0 ? current : bumped
  }
  const [major, minor, patch] = parse(released ?? current)
  if (level === 'major') return `${major + 1}.0.0`
  if (level === 'minor') return `${major}.${minor + 1}.0`
  if (level === 'patch') return `${major}.${minor}.${patch + 1}`
  return fail(`unknown bump «${level}»: auto, patch, minor, major or X.Y.Z`)
}

function replaceOnce(path, pattern, replacement) {
  const text = readFileSync(path, 'utf8')
  if (!pattern.test(text)) fail(`${path}: nothing to update (${pattern})`)
  writeFileSync(path, text.replace(pattern, replacement))
}

function bump(level = 'auto') {
  requireGit()
  const current = currentVersion()
  const next = nextVersion(level, current)
  const released = lastRelease()
  if (released && compare(next, released) <= 0) fail(`${next} is not newer than the last release ${released}`)
  const date = new Date().toISOString().slice(0, 10)

  // CHANGELOG: refresh the commits, then the whole [Unreleased] becomes [next].
  let text = withCommits(readFileSync(CHANGELOG, 'utf8'), commitList())
  if (text.includes(`## [${next}]`)) fail(`CHANGELOG.md already has a section for ${next}`)
  const { start, end } = unreleased(text)
  const body = text
    .slice(start, end)
    .replace('## [Unreleased]', '')
    .replace(`${START}\n\n`, '')
    .replace(`\n\n${END}`, '')
    .trim()
  const fresh = `## [Unreleased]\n\n### Коммиты\n\n${START}\n\n_Коммитов с прошлого релиза нет._\n\n${END}`
  text = `${text.slice(0, start)}${fresh}\n\n## [${next}] — ${date}\n\n${body}\n\n${text.slice(end)}`
  writeFileSync(CHANGELOG, text.replace(/\n{3,}/g, '\n\n'))

  // The same version everywhere version.sh and the bundles read it.
  replaceOnce(join(root, 'crates/app/tauri.conf.json'), /^ {2}"version": "[^"]+",$/m, `  "version": "${next}",`)
  replaceOnce(join(root, 'Cargo.toml'), /(\[workspace\.package\]\nversion = )"[^"]+"/, `$1"${next}"`)
  for (const crate of ['trade-app', 'trade-core']) {
    replaceOnce(join(root, 'Cargo.lock'), new RegExp(`(name = "${crate}"\\nversion = )"[^"]+"`), `$1"${next}"`)
  }
  const metainfo = join(root, 'packaging/flatpak/dev.mwt.MultiWalletTracker.metainfo.xml')
  if (readFileSync(metainfo, 'utf8').includes(`<release version="${next}"`)) {
    replaceOnce(metainfo, new RegExp(`<release version="${next.replaceAll('.', '\\.')}" date="[^"]*"/>`), `<release version="${next}" date="${date}"/>`)
  } else {
    replaceOnce(metainfo, /<releases>\n/, `<releases>\n    <release version="${next}" date="${date}"/>\n`)
  }

  console.log(`version ${current} → ${next}${released ? ` (last release ${released})` : ' (first release)'}`)
  console.log(`commit it: jj describe -m "chore(release): v${next}"`)
}

const [command, argument] = process.argv.slice(2)
if (command === 'changelog') changelog()
else if (command === 'bump') bump(argument)
else fail('usage: node scripts/release.mjs changelog | bump [auto|patch|minor|major|X.Y.Z]')
