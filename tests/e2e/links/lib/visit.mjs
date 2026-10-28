// What check-links does on one page: open a sample of its links, glances, and come back.
import {
  CLICK_BACK_BUTTON,
  CLICK_BACK_CHIP,
  CLICK_CARD_ACTION,
  CLICK_ENTITY,
  CLICK_GLANCE,
  CLICK_HREF,
  COLLECT,
  HAS_BACK_BUTTON,
  HAS_CARD,
  OPEN_PARAM,
  PAGE_STATE,
  PUSH_STACK,
  READ_BACK_CHIP,
  READ_GLANCE,
} from './probes.mjs'

const PER_KIND = 2
/** How many of the way-back probes ran (a probe that finds nothing to test counts nothing). */
export const stats = { stacks: 0, chips: 0, actions: 0 }

const wait = async (page, probe, tries = 20) => {
  for (let attempt = 0; attempt < tries; attempt++) {
    if (await page.eval(probe)) return true
    await page.sleep(150)
  }
  return false
}

async function escape(page) {
  for (const type of ['keyDown', 'keyUp']) {
    await page.send('Input.dispatchKeyEvent', { type, key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27 })
  }
  await page.sleep(450)
}

/** At most PER_KIND mentions of each kind, different ids. */
export function sample(entities) {
  const byKind = new Map()
  for (const entity of entities) {
    const kind = entity.key.split(':')[0]
    const list = byKind.get(kind) ?? []
    if (list.length < PER_KIND && !list.some((other) => other.key === entity.key)) list.push(entity)
    byKind.set(kind, list)
  }
  return [...byKind.values()].flat()
}

async function openOne(page, entity, at, errors) {
  // The page may have redirected (a loyalty wallet lands on the bonuses): back returns to where it landed.
  const { current: from, label: where } = at
  const label = `${where} ${entity.key} (${entity.open})`
  if (!(await page.eval(CLICK_ENTITY(entity.key, entity.open)))) return errors.push(`${label}: the link is gone after a re-render`)
  if (entity.open === 'card' || entity.open === 'tx') {
    if (!(await wait(page, HAS_CARD(entity.key)))) errors.push(`${label}: no card [data-entity-card="${entity.key}"]`)
    await escape(page)
  } else {
    await page.sleep(900)
    const state = await page.eval(PAGE_STATE)
    if (state.path === from) errors.push(`${label}: the route did not change`)
    else if (state.text < 10) errors.push(`${label}: empty page at ${state.path}`)
    else if (state.missing) errors.push(`${label}: «not found» page at ${state.path}`)
    await page.eval('history.back(); return true')
    await page.sleep(700)
  }
  const back = await page.eval(PAGE_STATE)
  if (back.path !== from) {
    errors.push(`${label}: back lands on ${back.path} instead of ${from}`)
    return false
  }
  return true
}

async function checkGlance(page, glance, where, errors) {
  const label = `${where} glance ${glance.kind} ${glance.key}`.trim()
  if (!(await page.eval(CLICK_GLANCE(glance.index)))) return
  await page.sleep(500)
  const body = await page.eval(READ_GLANCE)
  if (!body) errors.push(`${label}: does not open`)
  else if (body.text < 3) errors.push(`${label}: empty`)
  else if (!body.exit) errors.push(`${label}: no way out to a page (data-glance-open or a link)`)
  await escape(page)
}

/** The same screen: the lists add their own default period to the address, so only the path counts. */
const samePage = (left, right) => left.split('?')[0] === right.split('?')[0]

/** Two cards stacked (`?open=a~b`): the top one has «← Назад», it shows the one under; a lone card has none. */
async function checkStack(page, entities, at, errors) {
  const cards = sample(entities.filter((entity) => entity.open === 'card'))
  if (cards.length < 2) return false
  const [under, top] = cards
  stats.stacks++
  const label = `${at.label} stack ${under.key}~${top.key}`
  await page.eval(PUSH_STACK([under.key]))
  if (!(await wait(page, HAS_CARD(under.key)))) errors.push(`${label}: no card ${under.key}`)
  else if (await page.eval(HAS_BACK_BUTTON)) errors.push(`${label}: «← Назад» on a lone card`)
  await page.eval(PUSH_STACK([under.key, top.key]))
  if (!(await wait(page, HAS_CARD(top.key)))) errors.push(`${label}: no card ${top.key} on top`)
  else if (!(await wait(page, HAS_BACK_BUTTON, 6))) errors.push(`${label}: no «← Назад» (data-entity-back) over the other card`)
  else {
    await page.eval(CLICK_BACK_BUTTON)
    if (!(await wait(page, HAS_CARD(under.key)))) errors.push(`${label}: «← Назад» does not show ${under.key}`)
    else if ((await page.eval(OPEN_PARAM)) !== under.key) errors.push(`${label}: «← Назад» left open=${await page.eval(OPEN_PARAM)}`)
  }
  return true
}

/** A jump with `back=` lands on a list with the chip, and the chip returns to this page. */
async function checkBackChip(page, href, at, errors) {
  const label = `${at.label} back-chip via ${href.split('?')[0]}`
  if (!(await page.eval(CLICK_HREF(href)))) return true
  stats.chips++
  await page.sleep(900)
  const chip = await page.eval(READ_BACK_CHIP)
  if (chip === null) errors.push(`${label}: no «← назад» chip (data-back-chip) on the target`)
  else {
    await page.eval(CLICK_BACK_CHIP)
    await page.sleep(700)
    const state = await page.eval(PAGE_STATE)
    if (!samePage(state.path, at.current)) errors.push(`${label}: the chip «${chip}» lands on ${state.path} instead of ${at.current}`)
  }
  return samePage((await page.eval(PAGE_STATE)).path, at.current)
}

/** A card action that opens the operations list carries the way back too. */
async function checkCardAction(page, entities, at, errors) {
  const target = entities.find((entity) => entity.open === 'card' && /^(category|contact):/.test(entity.key))
  if (!target) return true
  const label = `${at.label} ${target.key} card action «operations»`
  await page.eval(CLICK_ENTITY(target.key, 'card'))
  if (!(await wait(page, HAS_CARD(target.key)))) return false
  if (!(await page.eval(CLICK_CARD_ACTION('operations')))) return true
  stats.actions++
  await page.sleep(900)
  const chip = await page.eval(READ_BACK_CHIP)
  if (chip === null) errors.push(`${label}: no «← назад» chip on the list`)
  else {
    await page.eval(CLICK_BACK_CHIP)
    await page.sleep(700)
    const state = await page.eval(PAGE_STATE)
    if (!samePage(state.path, at.current)) errors.push(`${label}: the chip lands on ${state.path} instead of ${at.current}`)
  }
  return samePage((await page.eval(PAGE_STATE)).path, at.current)
}

/** Visit the current page. Returns counts and appends to `errors` / `warnings`. */
export async function visitPage(page, at, errors, warnings, reopen) {
  const from = at.label
  at = { ...at, current: (await page.eval(PAGE_STATE)).path }
  const found = await page.eval(COLLECT)
  const seen = new Set()
  let checked = 0
  for (const link of found.links) {
    if (!link.internal || seen.has(link.href)) continue
    seen.add(link.href)
    checked++
    if (link.broken) errors.push(`${from}: link ${link.href} does not resolve in the router`)
    if (/^\/(stores\/|automations\/workflows\/)/.test(link.href) && !link.tagged) warnings.push(`${from}: ${link.href}`)
  }
  for (const entity of sample(found.entities)) {
    checked++
    if (!(await openOne(page, entity, at, errors))) await reopen()
  }
  if (found.entities.length) {
    checked++
    if (!(await checkStack(page, found.entities, at, errors))) checked--
    await reopen()
    checked++
    if (!(await checkCardAction(page, found.entities, at, errors))) await reopen()
    else await escape(page)
  }
  for (const href of found.backLinks.slice(0, 2)) {
    checked++
    if (!(await checkBackChip(page, href, at, errors))) await reopen()
  }
  for (const glance of found.glances.slice(0, 6)) {
    checked++
    await checkGlance(page, glance, from, errors)
  }
  return checked
}
