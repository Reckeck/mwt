import { type App, expect, test } from '../harness/app'

/**
 * Lock hygiene (spec lock-hygiene, tests 10 and 11): what the app learned from the vault is gone once it is
 * locked: the palette's Recent list and the query cache. The lock and the unlock run in place, in the same page.
 */
const LEGACY_KEYS = ['palette.recent', 'trade-app:recent-places']
const RECENT = '[data-palette-recent]'
const PALETTE = '[data-command-palette]'
const ROUTE = `const app = document.querySelector('#app').__vue_app__; const route = app.config.globalProperties.$route;`
const VAULT = `const vault = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('vault');`
const URL_OF = `const urlOf = (name) => performance.getEntriesByType('resource').map((entry) => entry.name).reverse().find((url) => url.includes(name)) ?? '/src/' + name`

/** Lock through the store, as the Lock button does, then open the same vault again without reloading the page. */
const lockAndUnlock = async (app: App) => {
  await app.page.eval(`${VAULT} await vault.lock(); return true`)
  await app.waitFor(`${VAULT} return vault.locked === true`, 'the vault to lock')
  await app.page.eval(`${VAULT} await vault.unlock('/tmp/mock.wallet', 'secret'); return true`)
  await app.waitFor(`${VAULT} return vault.locked === false`, 'the vault to unlock')
}

const openPalette = async (app: App) => {
  await app.click('[data-command-palette-trigger]')
  await app.waitFor(`return document.querySelector('${PALETTE}') !== null`, 'the palette to open')
}

const closePalette = async (app: App) => {
  await app.press('Escape')
  await app.waitFor(`return document.querySelector('${PALETTE}') === null`, 'the palette to close')
}

test('recents are empty after a lock, and legacy recents keys seeded before load are purged', async ({ app }) => {
  // The seed runs once, on the first document of the next navigation, before any app code.
  const flag = `seeded-${Date.now()}`
  const seed = `if (!sessionStorage.getItem('${flag}')) { sessionStorage.setItem('${flag}', '1'); ${LEGACY_KEYS.map((key) => `localStorage.setItem('${key}', '[{"title":"Secret wallet"}]')`).join('; ')} }`
  const { result } = await app.page.send('Page.addScriptToEvaluateOnNewDocument', { source: seed })
  try {
    await app.page.goto('about:blank')
    await app.unlock()
  } finally {
    await app.page.send('Page.removeScriptToEvaluateOnNewDocument', { identifier: result.identifier })
  }
  const seeded = await app.page.eval<string[]>(`return sessionStorage.getItem('${flag}') ? ['seen'] : []`)
  expect(seeded).toEqual(['seen'])
  for (const key of LEGACY_KEYS) {
    expect(await app.page.eval<string | null>(`return localStorage.getItem('${key}')`)).toBeNull()
  }

  // A card and a wallet are opened: both are in Recent, nothing of them reaches the disk.
  await app.at('/contacts')
  await app.clickText('[data-tour="contact-groups"] tbody tr', 'Олег')
  await app.waitFor(
    `${ROUTE} return Boolean(route.query.contact) && document.querySelector('[data-tour="contact-sheet"]') !== null`,
    'the card to open',
  )
  const storeId = await app.page.eval<string>(
    `${URL_OF}
     const { call } = await import(urlOf('shared/api/ipc.ts'))
     return (await call('get_store_tree'))[0].store.id`,
  )
  await app.at(`/stores/${storeId}`)
  await app.at('/')
  await openPalette(app)
  await app.waitFor(`return document.querySelector('${RECENT}') !== null`, 'Recent to list what was opened')
  await closePalette(app)
  for (const key of LEGACY_KEYS) {
    expect(await app.page.eval<string | null>(`return localStorage.getItem('${key}')`)).toBeNull()
  }

  await lockAndUnlock(app)
  await app.at('/')
  await openPalette(app)
  expect(await app.exists(RECENT)).toBe(false)
  await closePalette(app)
})

test('relocking the same vault refetches on the first render, nothing stale', async ({ app }) => {
  await app.unlock()
  await app.at('/contacts')
  await app.waitFor(`return document.querySelector('[data-tour="contact-groups"] tbody tr') !== null`, 'the contacts')
  const cached = `${URL_OF}
    const { queryClient } = await import(urlOf('app/providers/query/index.ts'))
    const queries = queryClient.getQueryCache().getAll();`
  expect(
    await app.page.eval<number>(`${cached} return queries.filter((query) => query.state.data !== undefined).length`),
  ).toBeGreaterThan(0)

  const lockedAt = await app.page.eval<number>(`${VAULT} const at = Date.now(); await vault.lock(); return at`)
  await app.waitFor(`${VAULT} return vault.locked === true`, 'the vault to lock')
  // The cache is dropped with the lock: not a row of the old session is left to show.
  expect(
    await app.page.eval<number>(`${cached} return queries.filter((query) => query.state.data !== undefined).length`),
  ).toBe(0)

  await app.page.eval(`${VAULT} await vault.unlock('/tmp/mock.wallet', 'secret'); return true`)
  await app.waitFor(`${VAULT} return vault.locked === false`, 'the vault to unlock')
  await app.at('/contacts')
  await app.waitFor(
    `return document.querySelector('[data-tour="contact-groups"] tbody tr') !== null`,
    'the contacts again',
  )
  // Every row on screen came from a fetch made after the lock.
  expect(
    await app.page.eval<number>(
      `${cached} return queries.filter((query) => query.state.data !== undefined && query.state.dataUpdatedAt < ${lockedAt}).length`,
    ),
  ).toBe(0)
  expect(
    await app.page.eval<number>(`${cached} return queries.filter((query) => query.state.data !== undefined).length`),
  ).toBeGreaterThan(0)
})
