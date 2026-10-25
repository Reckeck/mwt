// The links gate (stage 6½): from any screen a mention of an entity leads to its card or page and
// back; glances open and have a way out; every internal link resolves in the router. Runs the app
// (vite + mock `year`) in headless Chromium like check-layout.
//
//   node scripts/check-links.mjs            # gate: new errors past scripts/check-links.baseline.json fail
//   node scripts/check-links.mjs --update   # rewrite the baseline (it can only shrink by hand-review)
//   node scripts/check-links.mjs --quick    # 1280 only
// Warnings (a wallet or workflow link without data-entity) are listed, never fail.
import { existsSync, readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

import { findChromium, launch } from './check-layout/cdp.mjs'
import { login, open } from './check-layout/scenario.mjs'
import { freePort, startVite } from './check-layout/server.mjs'
import { IDS, ROUTES, WORKFLOW_IDS } from './check-links/probes.mjs'
import { stats, visitPage } from './check-links/visit.mjs'

const root = new URL('..', import.meta.url).pathname
const update = process.argv.includes('--update')
const quick = process.argv.includes('--quick')
const baselinePath = join(root, 'scripts/check-links.baseline.json')
const SKIP = new Set(['unlock', 'settings'])
// Phone width: the pages whose mentions differ most.
const PHONE_PAGES = new Set(['/', '/transactions'])

const binary = findChromium()
if (!binary) {
  console.error('check-links: no Chromium (set CHROMIUM=/path/to/chromium)')
  process.exit(1)
}

const { vite, url } = await startVite()
const page = await launch(binary, await freePort())
const errors = []
const warnings = []
let pages = 0
let links = 0

async function routesToVisit() {
  const named = await page.eval(ROUTES)
  const stores = await page.eval(IDS)
  await open(page, '/automations/workflows')
  const flows = await page.eval(WORKFLOW_IDS)
  const list = []
  for (const route of named) {
    if (SKIP.has(route.name)) continue
    if (!route.params.length) {
      list.push(route.path.startsWith('/') ? route.path : `/${route.path}`)
      continue
    }
    const pool = route.name === 'workflow-edit' ? flows.slice(0, 2) : stores.slice(0, 2)
    for (const id of pool) list.push(route.path.replace(':id', id))
    if (!pool.length) errors.push(`route ${route.name}: no id in the mock to open it with`)
  }
  return [...new Set(list)]
}

async function sweep(list, width, height) {
  await page.viewport(width, height, false)
  for (const path of list) {
    await open(page, path)
    const from = `${width} ${path}`
    pages++
    links += await visitPage(page, { path, label: from }, errors, warnings, () => open(page, path))
  }
}

try {
  await page.viewport(1280, 720, false)
  await page.goto(url)
  await login(page)
  const list = await routesToVisit()
  await sweep(list, 1280, 720)
  if (!quick) await sweep(list.filter((path) => PHONE_PAGES.has(path)), 390, 844)
} finally {
  await page.close()
  vite.kill()
}

// Errors are keyed by page and message minus ids that change nothing; the baseline holds the known ones.
const UUID = /[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/g
const key = (text) => text.replace(UUID, '<id>')
let known = []
if (existsSync(baselinePath)) known = JSON.parse(readFileSync(baselinePath, 'utf8'))
if (update) {
  writeFileSync(baselinePath, `${JSON.stringify([...new Set(errors.map(key))].sort(), null, 2)}\n`)
  console.log(`check-links: baseline written, ${new Set(errors.map(key)).size} entries`)
}
const fresh = update ? [] : errors.filter((text) => !known.includes(key(text)))
const gone = update ? [] : known.filter((text) => !errors.some((error) => key(error) === text))
for (const text of fresh) console.error(`error: ${text}`)
for (const text of gone) console.log(`fixed (remove from the baseline with --update): ${text}`)
const unique = [...new Set(warnings.map((text) => key(text).replace(/^\d+ /, '')))]
if (unique.length) console.log(`warnings: ${warnings.length} links without data-entity, ${unique.length} unique pairs`)
for (const text of unique) console.log(`warn: ${text}`)
console.log(`way back: ${stats.stacks} card stacks, ${stats.chips} drill chips, ${stats.actions} card actions`)
console.log(`check-links: ${pages} pages, ${links} links checked, ${errors.length} errors (${fresh.length} new), ${warnings.length} warnings`)
process.exit(fresh.length ? 1 : 0)
