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

function crypto(symbol: string, quantity: string, valueBase: string): StorePosition {
  return {
    asset: { symbol, chain: 'bitcoin', contract: null },
    quantity,
    value_base: valueBase,
    price_base: valueBase,
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

  it('gives a single non-base fiat position its own currency, the base currency and the rate', () => {
    const figure = walletFigure([fiat('EUR', '10', '1000', '100')], '1000', 'rub')
    expect(figure.amount).toBe('10')
    expect(figure.currency).toBe('eur')
    expect(figure.base).toEqual({ amount: '1000', rate: 1 / 100, currency: 'rub' })
  })

  it('gives a zero rate when the price is missing', () => {
    expect(walletFigure([fiat('EUR', '10', '0', '0')], '0', 'rub').base?.rate).toBe(0)
  })

  it('shows the base sum for a single base-fiat position', () => {
    const figure = walletFigure([fiat('RUB', '10', '10', '1')], '10', 'rub')
    expect(figure).toMatchObject({ amount: '10', currency: 'rub', base: null, assets: ['RUB'] })
  })

  it('shows the base sum and the symbol for a single crypto position', () => {
    const figure = walletFigure([crypto('BTC', '1', '5000')], '5000', 'rub')
    expect(figure).toEqual({ amount: '5000', currency: 'rub', base: null, assets: ['BTC'] })
  })

  it('lists several positions biggest first and shows the base sum', () => {
    const figure = walletFigure([crypto('BTC', '1', '5000'), fiat('EUR', '10', '1000', '100'), crypto('ETH', '1', '9000')], '15000', 'rub')
    expect(figure.assets).toEqual(['ETH', 'BTC', 'EUR'])
    expect(figure.currency).toBe('rub')
    expect(figure.base).toBeNull()
  })

  it('ignores zero quantities: one held foreign fiat still shows its own currency', () => {
    const figure = walletFigure([fiat('EUR', '10', '1000', '100'), crypto('BTC', '0', '0')], '1000', 'rub')
    expect(figure.currency).toBe('eur')
    expect(figure.assets).toEqual(['EUR'])
  })

  it('has no assets and the base sum when every quantity is zero', () => {
    const figure = walletFigure([fiat('EUR', '0', '0', '100'), crypto('BTC', '0', '0')], '0', 'rub')
    expect(figure).toEqual({ amount: '0', currency: 'rub', base: null, assets: [] })
  })
})
