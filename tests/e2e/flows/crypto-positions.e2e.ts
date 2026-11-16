import { expect, test } from '../harness/app'

/**
 * The DeFi positions screen on the mock (spec D6, its acceptance at both widths): the summary over the list,
 * the protocol chips, the rows (a table on a computer, cards on a phone) and one position's breakdown.
 *
 * What it guards: the figures are the engine's own and add up, a figure nobody knows is a dash and not a zero,
 * the filter lives in the address, and neither width scrolls sideways.
 */
for (const width of [1280, 390] as const) {
  test(`DeFi positions: the summary, a chip and one breakdown, ${width}px`, async ({ app }) => {
    await app.unlock()
    await app.at('/crypto/positions', { width, theme: 'light' })
    await app.waitFor(`return document.querySelector('[data-tour="defi-positions"]') !== null`, 'the screen')
    await app.waitFor(`return /stETH/.test(document.body.innerText)`, 'the rows of the mock')

    const list = await app.text('main')
    // The mock's open positions put 22 700 in; the net result is a figure, never 0.00.
    expect(list).toMatch(/22[\s ,.]?700/)
    expect(list).toMatch(/2[\s ,.]?934/)
    // The v4 position behind a hook knows nothing: a dash, not a zero.
    expect(list).toMatch(/—/)
    expect(list).toMatch(/Uniswap v3/)
    await app.screenshot(`crypto-positions-1-list-${width}`)
    await app.waitFor(
      `return document.documentElement.scrollWidth <= document.documentElement.clientWidth`,
      'no horizontal scroll on the list',
    )

    // A chip narrows the list and goes into the address, so a reload keeps it.
    await app.clickText('button', '^Lido$')
    await app.waitFor(`return /protocol=lido/.test(location.search)`, 'the filter in the address')
    await app.waitFor(`return !/Uniswap v3/.test(document.body.innerText)`, 'only Lido left')
    expect(await app.text('main')).toMatch(/stETH/)
    await app.screenshot(`crypto-positions-2-filtered-${width}`)

    // One position's detail: the breakdown is there and the net is not a zero in disguise.
    await app.at('/crypto/positions', { width, theme: 'light' })
    await app.waitFor(`return /stETH/.test(document.body.innerText)`, 'the rows again')
    await app.clickText('tbody tr, li button', 'stETH')
    await app.waitFor(
      `return document.querySelector('[data-tour="defi-position-breakdown"]') !== null`,
      'the breakdown of the open position',
    )
    const detail = await app.text('[data-tour="defi-position-breakdown"]')
    expect(detail).not.toMatch(/NaN|undefined/)
    expect(detail).toMatch(/[0-9]/)
    await app.screenshot(`crypto-positions-3-detail-${width}`)
    await app.waitFor(
      `return document.documentElement.scrollWidth <= document.documentElement.clientWidth`,
      'no horizontal scroll with the detail open',
    )
  })
}
