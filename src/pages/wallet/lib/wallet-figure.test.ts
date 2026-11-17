import { describe, expect, it } from 'vitest'

import { walletFigure } from './wallet-figure'

function fiat(symbol: string, quantity: string, valueBase: string, priceBase: string): StorePosition {
  return {
    asset: { symbol, chain: 'fiat', contract: null },
    quantity,
    value_base: valueBase,
    price_base: priceBase,
  } as unknown as StorePosition
}

describe('walletFigure', () => {
  it('carries the overview base currency on the base line of a foreign-currency wallet', () => {
    const figure = walletFigure([fiat('EUR', '10', '1000', '100')], '1000', 'rub')
    expect(figure.currency).toBe('eur')
    expect(figure.base?.currency).toBe('rub')
  })

  it('shows the base sum in the base currency otherwise', () => {
    const figure = walletFigure([fiat('RUB', '10', '10', '1')], '10', 'rub')
    expect(figure.currency).toBe('rub')
    expect(figure.base).toBeNull()
  })
})
