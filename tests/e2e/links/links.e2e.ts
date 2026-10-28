// The links gate (stage 6.5): from any screen a mention of an entity leads to its card or page and back;
// glances open and have a way out; every internal link resolves in the router. Runs the app (Vite + mock)
// in headless Chromium like check-layout.
//
//   mise run check-links                 # gate: new errors past scripts/check-links.baseline.json fail
//   mise run check-links -- --update     # rewrite the baseline          (E2E_UPDATE=1)
//   mise run check-links -- --quick      # 1280 only                     (E2E_QUICK=1)
// Warnings (a wallet or workflow link without data-entity) are listed, never fail.
import { existsSync, readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

import { afterAll, beforeAll, describe, inject, it } from 'vitest'

import { connect } from '../harness/app'
import type { Page } from '../harness/cdp.mjs'
import { login, open } from '../layout/lib/scenario.mjs'
import { IDS, ROUTES, WORKFLOW_IDS } from './lib/probes.mjs'
import { stats, visitPage } from './lib/visit.mjs'

const ROOT = new URL('../../..', import.meta.url).pathname
const BASELINE = join(ROOT, 'scripts/check-links.baseline.json')
const update = process.env.E2E_UPDATE === '1'
const quick = process.env.E2E_QUICK === '1'
const SKIP = new Set(['unlock', 'settings'])
// Phone width: the pages whose mentions differ most.
const PHONE_PAGES = new Set(['/', '/transactions'])
// Errors are keyed by page and message minus ids that change nothing; the baseline holds the known ones.
const UUID = /[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/g
const key = (text: string) => text.replace(UUID, '<id>')

describe('check-links', () => {
  let app: Awaited<ReturnType<typeof connect>>
  let page: Page
  const errors: string[] = []
  const warnings: string[] = []
  let pages = 0
  let links = 0
  let list: string[] = []

  async function routesToVisit() {
    const named: { name: string; path: string; params: string[] }[] = await page.eval(ROUTES)
    const stores: string[] = await page.eval(IDS)
    await open(page, '/automations/workflows')
    const flows: string[] = await page.eval(WORKFLOW_IDS)
    const out: string[] = []
    for (const route of named) {
      if (SKIP.has(route.name)) continue
      if (!route.params.length) {
        out.push(route.path.startsWith('/') ? route.path : `/${route.path}`)
        continue
      }
      const pool = route.name === 'workflow-edit' ? flows.slice(0, 2) : stores.slice(0, 2)
      for (const id of pool) out.push(route.path.replace(':id', id))
      if (!pool.length) errors.push(`route ${route.name}: no id in the mock to open it with`)
    }
    return [...new Set(out)]
  }

  async function sweep(paths: string[], width: number, height: number) {
    await page.viewport(width, height, false)
    for (const path of paths) {
      await open(page, path)
      pages++
      links += await visitPage(page, { path, label: `${width} ${path}` }, errors, warnings, () => open(page, path))
    }
  }

  beforeAll(async () => {
    app = await connect({ animations: true })
    page = app.page
    await page.viewport(1280, 720, false)
    await page.goto(inject('e2e').url)
    await login(page)
    list = await routesToVisit()
  }, 120_000)

  afterAll(async () => {
    await app?.close()
  })

  it('visits every page at 1280', async () => {
    await sweep(list, 1280, 720)
  }, 600_000)

  it.skipIf(quick)(
    'visits the main pages at 390',
    async () => {
      await sweep(
        list.filter((path) => PHONE_PAGES.has(path)),
        390,
        844,
      )
    },
    600_000,
  )

  it('has no new errors past the baseline', () => {
    const known: string[] = existsSync(BASELINE) ? JSON.parse(readFileSync(BASELINE, 'utf8')) : []
    if (update) {
      const next = [...new Set(errors.map(key))].sort()
      writeFileSync(BASELINE, `${JSON.stringify(next, null, 2)}\n`)
      console.log(`check-links: baseline written, ${next.length} entries`)
      return
    }
    const fresh = errors.filter((text) => !known.includes(key(text)))
    for (const text of known.filter((text) => !errors.some((error) => key(error) === text))) {
      console.log(`fixed (remove from the baseline with --update): ${text}`)
    }
    const unique = [...new Set(warnings.map((text) => key(text).replace(/^\d+ /, '')))]
    if (unique.length)
      console.log(`warnings: ${warnings.length} links without data-entity, ${unique.length} unique pairs`)
    for (const text of unique) console.log(`warn: ${text}`)
    console.log(`way back: ${stats.stacks} card stacks, ${stats.chips} drill chips, ${stats.actions} card actions`)
    console.log(
      `check-links: ${pages} pages, ${links} links checked, ${errors.length} errors (${fresh.length} new), ${warnings.length} warnings`,
    )
    if (fresh.length) throw new Error(`\n${fresh.map((text) => `error: ${text}`).join('\n')}`)
  })
})
