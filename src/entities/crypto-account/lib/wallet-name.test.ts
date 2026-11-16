import { describe, expect, it } from 'vitest'

import { defaultWalletName, uniqueWalletName } from './wallet-name'

const ADDRESS = '0x7a3F19c4D2b85E6F0a9C41b7cE3a0D8e2f4b9bC1'

describe('defaultWalletName', () => {
  it('is the network and the short address', () => {
    expect(defaultWalletName('Ethereum', ADDRESS, [])).toBe('Ethereum · 0x7a3F…9bC1')
  })

  it('is the network alone while there is no address', () => {
    expect(defaultWalletName('Base', '  ', [])).toBe('Base')
  })

  it('differs for two addresses on one network', () => {
    const first = defaultWalletName('Ethereum', ADDRESS, [])
    const second = defaultWalletName('Ethereum', '0x52908400098527886E0F7030069857D2E4169EE7', [first])
    expect(second).not.toBe(first)
    expect(second).toBe('Ethereum · 0x5290…9EE7')
  })

  it('adds a suffix when the name is taken, ignoring case and spaces', () => {
    expect(defaultWalletName('Ethereum', ADDRESS, [' ethereum · 0X7A3F…9BC1 '])).toBe('Ethereum · 0x7a3F…9bC1 (2)')
  })
})

describe('uniqueWalletName', () => {
  it('counts up past every taken suffix', () => {
    expect(uniqueWalletName('Main', ['Main', 'Main (2)'])).toBe('Main (3)')
  })

  it('keeps a free name', () => {
    expect(uniqueWalletName('Main', ['Other'])).toBe('Main')
  })
})
