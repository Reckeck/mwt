import { type App, expect, test } from '../harness/app'

const SKEWERS = '/money/together/mock-event-skewers'
const TOTALS = '[data-tour="shared-totals"]'
const PEOPLE_FIELD = 'input[aria-label="Add a person: name or contact"]'
const EVENT_DIALOG = '[data-overlay="shared-event-edit"]'

/** The three totals as numbers: `{ planned: 9400, spent: 7100, diff: 600 }`. */
async function totals(app: App) {
  const text = await app.page.eval<string>(`
    const cards = [...document.querySelectorAll('${TOTALS} [data-slot="stat-card"], ${TOTALS} > div > *')]
    return cards.map((card) => card.textContent.replace(/\\s+/g, ' ').trim()).join('|')`)
  const read = (label: RegExp) => {
    const match = text.match(label)
    return match ? Number(match[1].replace(/[\s,  ]/g, '')) : Number.NaN
  }
  return {
    planned: read(/Planned\s*([+−-]?[\d\s,  ]+)/),
    spent: read(/Spent\s*([+−-]?[\d\s,  ]+)/),
    diff: read(/Difference\s*([+−-]?[\d\s,  ]+)/),
  }
}

async function openSkewers(app: App) {
  await app.unlock()
  await app.at(SKEWERS)
  await app.waitFor(`return document.querySelector('${TOTALS}') !== null`, 'the event totals')
}

test('the seed event shows the planned, spent and difference of the board', async ({ app }) => {
  await openSkewers(app)
  expect(await totals(app)).toEqual({ planned: 9400, spent: 7100, diff: 600 })
  const items = await app.text('[data-tour="shared-items-table"]')
  expect(items).toContain('Мясо')
  expect(items).toContain('Овощи')
  expect(items).toMatch(/off plan/i)
})

test('marking the vegetables as bought adds their sum to the spent total', async ({ app }) => {
  await openSkewers(app)
  const before = await totals(app)

  await app.page.eval(`
    const row = [...document.querySelectorAll('tr')].find((entry) => /Овощи/.test(entry.textContent))
    row.querySelector('[data-tour="shared-buy"]').click()
    return true`)
  await app.waitFor(`return document.querySelector('[data-overlay="shared-purchase"]') !== null`, 'the purchase dialog')
  // The dialog starts from the plan: 3 kg for 900.
  expect(await app.value('#purchase-qty')).toBe('3')
  expect((await app.value('#purchase-amount')).replace(/\s/g, '')).toBe('900')
  await app.clickText('[data-overlay="shared-purchase"] button', '^Save$')
  await app.waitFor(`return document.querySelector('[data-overlay="shared-purchase"]') === null`, 'the dialog to close')

  await app.waitFor(
    `return /8[\\s,\\u00a0\\u202f]?000/.test(document.querySelector('${TOTALS}')?.textContent ?? '')`,
    'Spent to become 8 000',
  )
  const after = await totals(app)
  expect(after.spent).toBe(before.spent + 900)
  expect(after.planned).toBe(before.planned)
  // Bought at the planned price: the overspend of the beer stays what it was.
  expect(after.diff).toBe(before.diff)
  // The vegetables no longer wait for a purchase.
  const row = await app.page.eval<string>(
    `return [...document.querySelectorAll('tr')].find((entry) => /Овощи/.test(entry.textContent)).textContent.replace(/\\s+/g, ' ')`,
  )
  expect(row).toMatch(/bought/i)
  expect(row).not.toMatch(/Bought\s*$/)
})

test('a new event with two participants appears in the list', async ({ app }) => {
  await app.unlock()
  await app.at('/money/together')
  await app.click('[data-tour="shared-new"]')
  await app.waitFor(`return document.querySelector('${EVENT_DIALOG}') !== null`, 'the new event dialog')
  await app.type('#event-title', 'Поход')

  for (const name of ['Петя', 'Вася']) {
    await app.type(PEOPLE_FIELD, name)
    await app.clickText('[role="option"]', `^${name}$`)
    await app.waitFor(
      `return [...document.querySelectorAll('${EVENT_DIALOG} td')].some((cell) => cell.textContent.includes('${name}'))`,
      `${name} among the participants`,
    )
  }
  await app.waitFor(
    `return /People · 3/.test(document.querySelector('${EVENT_DIALOG}')?.textContent ?? '')`,
    'me and two people',
  )

  await app.clickText(`${EVENT_DIALOG} button`, '^Create$')
  await app.waitFor(`return document.querySelector('${EVENT_DIALOG}') === null`, 'the dialog to close')
  // The new event opens on its own screen with the two people in the shares line.
  await app.waitFor(
    `return /Поход/.test(document.querySelector('h1, main')?.textContent ?? '') && document.querySelector('${TOTALS}') !== null`,
    'the event screen',
  )
  const shares = await app.text('[data-tour="shared-shares"]')
  expect(shares).toContain('Петя')
  expect(shares).toContain('Вася')

  await app.at('/money/together')
  await app.waitFor(
    `return /Поход/.test(document.querySelector('[data-tour="shared-list"]')?.textContent ?? '')`,
    'the new event in the list',
  )
  const list = await app.text('[data-tour="shared-list"]')
  expect(list).toContain('Шашлыки')
  expect(list).toContain('Поход')
})
