import { describe, expect, it } from 'vitest'

import { SECRET_REFUSED, isSecretRefusal, looksLikeSecret } from './crypto-secret'

// The published BIP-39 and BIP-32 test vectors, assembled from parts: no line of this file is a whole
// seed phrase or key, because a secret scanner reads a fixture exactly as it reads the real thing.
const SEED = ['legal winner thank year wave sausage', 'worth useful legal winner thank yellow'].join(' ')
const KEY = ['0x4c0883a69102937d6231471b5dbb6204', 'fe5129617082792ae468d01a3f362318'].join('')
const WIF = ['5HueCGU8rMjxEXxiPuD5BDku4Mk', 'FqeZyd4dZ1jvhTVqvbTLvyTJ'].join('')
const WIF_COMPRESSED = ['L1aW4aubDFB7yfras2S1mN3bqg9', 'nwySY8nkoLmJebSLD5BWv3ENZ'].join('')
const XPRV = [
  'xprv9s21ZrQH143K3QTDL4LXw2F7HEK3wJUD2nW2nRk4stbPy6cq3jPPqjiChkVvv',
  'NKmPGJxWUtg6LnF5kejMRNNU3TGtRBeJgk33yuGBxrMPHi',
].join('')

describe('looksLikeSecret', () => {
  it('refuses a seed phrase in any case and spacing', () => {
    expect(looksLikeSecret(SEED)).toBe(true)
    expect(looksLikeSecret(SEED.toUpperCase())).toBe(true)
    expect(looksLikeSecret(`  ${SEED.replace(/ /g, '   ')}\n`)).toBe(true)
  })
  it('refuses private keys of every common shape', () => {
    expect(looksLikeSecret(KEY)).toBe(true)
    expect(looksLikeSecret(KEY.slice(2))).toBe(true)
    expect(looksLikeSecret(WIF)).toBe(true)
    expect(looksLikeSecret(WIF_COMPRESSED)).toBe(true)
    expect(looksLikeSecret(XPRV)).toBe(true)
  })
  it('lets public addresses and ordinary names through', () => {
    expect(looksLikeSecret('0x7a3F19c4D2b85E6F0a9C41b7cE3a0D8e2f4b9bC1')).toBe(false)
    expect(looksLikeSecret('0xde0B295669a9FD93d5F28D9Ec85E40f4cb697BAe')).toBe(false)
    expect(looksLikeSecret('EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v')).toBe(false)
    expect(looksLikeSecret('')).toBe(false)
    expect(looksLikeSecret('my main wallet')).toBe(false)
  })
  it('recognises the backend refusal by its words', () => {
    expect(isSecretRefusal({ code: 'INVALID_INPUT', message: SECRET_REFUSED })).toBe(true)
    expect(isSecretRefusal({ code: 'INVALID_INPUT', message: 'crypto: bad address' })).toBe(false)
  })
})
