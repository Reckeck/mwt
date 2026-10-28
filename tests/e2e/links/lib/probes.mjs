// Code that runs inside the page (async function bodies for `page.eval`).

const APP = `const app = document.querySelector('#app').__vue_app__; const gp = app.config.globalProperties;`

/** Named routes of the router; `params` lists the dynamic segments. */
export const ROUTES = `${APP}
  return gp.$router.getRoutes().filter((route) => route.name).map((route) => ({
    name: String(route.name),
    path: route.path,
    params: [...route.path.matchAll(/:(\\w+)/g)].map((match) => match[1]),
  }))`

/** Ids of the mock: wallets of each kind, workflows (from the list screen). */
export const IDS = `
  const { call } = await import('/src/shared/api/ipc.ts')
  const tree = await call('get_store_tree')
  const seen = new Set()
  const stores = []
  for (const node of tree) {
    const kind = node.store?.kind ?? node.kind
    if (seen.has(kind)) continue
    seen.add(kind)
    stores.push(node.store?.id ?? node.id)
  }
  return stores`

export const WORKFLOW_IDS = `
  const ids = [...document.querySelectorAll('a[href^="/automations/workflows/"]')]
    .map((link) => link.getAttribute('href').split('/')[3].split('?')[0])
    .filter((id) => id && id !== 'new')
  return [...new Set(ids)]`

/** Where we are, and whether the page shows something and not an error. */
export const PAGE_STATE = `${APP}
  const main = document.querySelector('main')
  const text = (main?.innerText ?? '').trim()
  return { path: gp.$route.fullPath, name: String(gp.$route.name ?? ''), text: text.length, missing: /не найден|not found/i.test(text) }`

/** Links and entity mentions on the page. */
export const COLLECT = `${APP}
  const visible = (el) => { const box = el.getBoundingClientRect(); return box.width > 0 && box.height > 0 }
  const entities = [...document.querySelectorAll('[data-entity-open]')].filter(visible).map((el) => ({
    key: el.getAttribute('data-entity'),
    open: el.getAttribute('data-entity-open'),
  }))
  // A transaction row has no id in the DOM: one row stands for all, its card is tx:any.
  if (document.querySelector('[data-tx-open]')) entities.push({ key: 'tx:*', open: 'tx' })
  const links = [...document.querySelectorAll('a[href]')].map((el) => {
    const href = el.getAttribute('href')
    let broken = false
    if (href.startsWith('/')) {
      const resolved = gp.$router.resolve(href)
      broken = resolved.matched.length === 0 || resolved.matched.some((record) => record.path.includes('pathMatch'))
    }
    return { href, internal: href.startsWith('/'), broken, tagged: Boolean(el.closest('[data-entity]')) }
  })
  const glances = [...document.querySelectorAll('[data-glance-trigger]')].filter(visible).map((el, index) => ({
    index, kind: el.getAttribute('data-glance-trigger'), key: el.getAttribute('data-entity') ?? '',
  }))
  const backLinks = [...new Set([...document.querySelectorAll('a[href*="back="]')].filter(visible).map((el) => el.getAttribute('href')))]
  return { entities, links, glances, backLinks }`

export const CLICK_ENTITY = (key, open) => `
  if (${JSON.stringify(open)} === 'tx') {
    const row = [...document.querySelectorAll('[data-tx-open]')].find((node) => node.getBoundingClientRect().width > 0)
    if (!row) return false
    row.click()
    return true
  }
  const el = [...document.querySelectorAll('[data-entity-open="${open}"]')].find((node) => node.getAttribute('data-entity') === ${JSON.stringify(key)})
  if (!el) return false
  el.click()
  return true`

export const HAS_CARD = (key) =>
  key.endsWith(':*')
    ? `return Boolean(document.querySelector('[data-entity-card^=${JSON.stringify(key.slice(0, -1))}]'))`
    : `return Boolean(document.querySelector('[data-entity-card=${JSON.stringify(key)}]'))`

export const CLICK_GLANCE = (index) => `
  const el = [...document.querySelectorAll('[data-glance-trigger]')].filter((node) => {
    const box = node.getBoundingClientRect(); return box.width > 0 && box.height > 0
  })[${index}]
  if (!el) return false
  el.click()
  return true`

/** The open glance: its text length and the way out to a page. */
export const READ_GLANCE = `
  const body = document.querySelector('[data-glance]')
  if (!body) return null
  return { text: body.innerText.trim().length, exit: Boolean(body.querySelector('[data-glance-open], a[href]')) }`


/** Push a stack of cards onto the current page: `open=a~b`. */
export const PUSH_STACK = (keys) => `${APP}
  const { $route: route, $router: router } = gp
  await router.push({ path: route.path, query: { ...route.query, open: ${JSON.stringify(keys.join('~'))} } })
  return true`

export const OPEN_PARAM = `${APP} return gp.$route.query.open ?? ''`

export const HAS_BACK_BUTTON = `return Boolean(document.querySelector('[data-entity-back]'))`

export const CLICK_BACK_BUTTON = `
  const el = document.querySelector('[data-entity-back]')
  if (!el) return false
  el.click()
  return true`

/** A link made by drillTo / a card action: it carries `back=`. */
export const CLICK_HREF = (href) => `
  const el = [...document.querySelectorAll('a[href]')].find((node) => node.getAttribute('href') === ${JSON.stringify(href)})
  if (!el) return false
  el.click()
  return true`

/** The «← назад» chip on the page we landed on: its label, or null. */
export const READ_BACK_CHIP = `
  const chip = document.querySelector('[data-back-chip]')
  return chip ? chip.innerText.trim() : null`

export const CLICK_BACK_CHIP = `
  const chip = document.querySelector('[data-back-chip]')
  if (!chip) return false
  chip.click()
  return true`

/** The card's own action by id (`data-entity-action`). */
export const CLICK_CARD_ACTION = (id) => `
  const el = document.querySelector('[data-entity-card] [data-entity-action="${id}"]')
  if (!el) return false
  el.click()
  return true`
