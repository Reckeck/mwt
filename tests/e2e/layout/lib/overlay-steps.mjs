// Building blocks of the overlay check: placeholders, clicks, keys, finding a layer, closing it.
export const APP = `const app = document.querySelector('#app').__vue_app__; const gp = app.config.globalProperties;`

/** The page under a layer: its size and the main column's box. */
export const PAGE = `
  const main = document.querySelector('main') ?? document.body
  const rect = main.getBoundingClientRect()
  const root = document.documentElement
  return [root.scrollWidth, root.scrollHeight, root.clientWidth, Math.round(rect.left), Math.round(rect.top), Math.round(rect.width), Math.round(rect.height)]
`

/** Placeholders in routes and query values: {goal}, {automation}, {store:<kind>}, {event:seed|others} (the seed event; an event somebody else leads). */
export async function fill(page, text) {
  let out = text
  for (const [token] of text.matchAll(/\{[a-z]+(?::[a-z_]+)?\}/g)) {
    const [name, arg] = token.slice(1, -1).split(':')
    const value = await page.eval(`
      const { call } = await import('/src/shared/api/ipc.ts')
      if ('${name}' === 'goal') return (await call('list_goals'))[0]?.id ?? ''
      if ('${name}' === 'store') {
        const tree = await call('get_store_tree')
        const node = tree.find((item) => (item.store?.kind ?? item.kind) === '${arg}') ?? tree[0]
        return node?.store?.id ?? node?.id ?? ''
      }
      if ('${name}' === 'event') {
        if ('${arg}' !== 'others') return 'mock-event-skewers'
        // «Pay my share» exists only where somebody else leads: make one (once) in which she bought for the three of us.
        const found = (await call('list_shared_events', { filter: { include_archived: false } })).find((entry) => entry.title === 'Дача')
        if (found) return found.id
        const guest = (id, name) => ({ id, name, contact_id: null, details: null })
        const view = await call('create_shared_event', { input: {
          title: 'Дача', note: '', starts_on: null, ends_on: null, currency: 'rub', rounding_step: '1', category_id: null,
          organizer: 'anna', i_participate: true, participants: [guest('anna', 'Анна'), guest('boris', 'Борис')],
        } })
        await call('add_shared_purchase', { input: { event_id: view.event.id, purchase: {
          item_id: null, new_item_title: 'Продукты', new_item_unit: 'шт', buyer: 'anna', qty: '1', amount: '900', currency: 'rub',
          price_mode: 'total', rate_to_event: null, at: null, store_id: null,
        } } })
        return view.event.id
      }
      if ('${name}' === 'automation') return (await call('list_automations'))[0]?.id ?? ''
      return ''`)
    out = out.replace(token, value)
  }
  return out
}

/** The first visible element of `selector` whose text matches `text`, as a click point. */
export async function pointOf(page, selector, text) {
  return page.eval(`
    const re = ${text ? `new RegExp(${JSON.stringify(text)}, 'i')` : 'null'}
    const el = [...document.querySelectorAll(${JSON.stringify(selector)})].find((node) => {
      const rect = node.getBoundingClientRect()
      return rect.width && rect.height && getComputedStyle(node).visibility !== 'hidden' && (!re || re.test(node.textContent + ' ' + (node.getAttribute('aria-label') ?? '')))
    })
    if (!el) return null
    el.scrollIntoView({ block: 'center', inline: 'center' })
    await new Promise((done) => requestAnimationFrame(done))
    const rect = el.getBoundingClientRect()
    // A see-through button (an openable row) can have a chip with its own action over its centre:
    // press where the element itself is on top, as a person would.
    const hits = (x, y) => { const top = document.elementFromPoint(x, y); return top && (top === el || el.contains(top)) }
    for (const fx of [0.5, 0.15, 0.85, 0.3, 0.7]) {
      for (const fy of [0.5, 0.25, 0.75]) {
        const x = Math.round(rect.left + rect.width * fx), y = Math.round(rect.top + rect.height * fy)
        if (hits(x, y)) return [x, y]
      }
    }
    return [Math.round(rect.left + rect.width / 2), Math.round(rect.top + rect.height / 2)]`)
}

export async function press(page, key, modifiers = 0) {
  const info = { Escape: [27, 'Escape'], k: [75, 'KeyK'], n: [78, 'KeyN'] }[key]
  for (const type of ['rawKeyDown', 'keyUp']) {
    await page.send('Input.dispatchKeyEvent', { type, key, code: info[1], windowsVirtualKeyCode: info[0], modifiers })
  }
}

/** One open step: { click | context | hover, text? } | { key } | { query } | { eval }. */
export async function step(page, item) {
  if (item.click || item.context || item.hover) {
    const selector = item.click ?? item.context ?? item.hover
    const at = await pointOf(page, selector, item.text)
    if (!at) return `nothing to ${item.click ? 'click' : item.context ? 'right-click' : 'hover'}: ${selector}${item.text ? ` "${item.text}"` : ''}`
    await page.mouse(at[0], at[1])
    if (!item.hover) {
      const button = item.context ? 'right' : 'left'
      await page.send('Input.dispatchMouseEvent', { type: 'mousePressed', x: at[0], y: at[1], button, clickCount: 1 })
      await page.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: at[0], y: at[1], button, clickCount: 1 })
    }
  } else if (item.key) {
    await press(page, item.key.split('+').at(-1).toLowerCase() === 'k' ? 'k' : 'n', 2)
  } else if (item.query) {
    const query = JSON.stringify(item.query)
    const filled = await fill(page, query)
    await page.eval(`${APP}
      const route = gp.$route
      await gp.$router.push({ path: route.path, query: { ...route.query, ...${filled} } })
      return true`)
  } else if (item.eval) {
    await page.eval(item.eval)
  }
  await page.sleep(item.wait ?? 120)
  return null
}

/** The layer's box if it is on screen and not closing, else null. */
export const FIND = (selector) => `
  const layer = [...document.querySelectorAll(${JSON.stringify(selector)})].find((node) => {
    const rect = node.getBoundingClientRect()
    return rect.width && rect.height && node.getAttribute('data-state') !== 'closed' && getComputedStyle(node).visibility !== 'hidden'
  })
  `
export const LAYER = (selector) => `${FIND(selector)}
  if (!layer) return null
  const rect = layer.getBoundingClientRect()
  return [Math.round(rect.left), Math.round(rect.top), Math.round(rect.width), Math.round(rect.height)]`

export const differs = (left, right, slack = 1) => left.some((value, index) => Math.abs(value - right[index]) > slack)

export async function openLayer(page, entry) {
  // A tooltip left open by the pointer of the last entry would take the Esc meant for the layer.
  await page.mouse(2, 2)
  await page.eval(`document.activeElement?.blur(); return true`)
  // `fresh`: the layer opens when its page mounts (?edit=, a tour): arrive from another page.
  if (entry.fresh) await page.eval(`${APP} await gp.$router.push('/stores'); return true`)
  await page.eval(`${APP} await gp.$router.push(${JSON.stringify(await fill(page, entry.route))}); return true`)
  await page.sleep(400)
  await page.eval(`
    document.querySelector('.driver-popover-close-btn')?.click()
    for (const button of document.querySelectorAll('[role="dialog"] button')) {
      if (/^(Понятно|Got it)$/.test(button.textContent.trim())) button.click()
    }
    return true`)
  await page.sleep(200)
}


/** Esc with a mouse, `Back` on a phone; waits until the layer is gone. Returns whether it went. */
export async function closeLayer(page, selector, touch) {
  if (touch) await page.eval(`const { closeTopLayer } = await import('/src/shared/lib/back-stack.ts'); return closeTopLayer()`)
  else await press(page, 'Escape')
  for (let tries = 0; tries < 30; tries++) {
    await page.sleep(50)
    if (!(await page.eval(LAYER(selector)))) return true
  }
  return false
}
