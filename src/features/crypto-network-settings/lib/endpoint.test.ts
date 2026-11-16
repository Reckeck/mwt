import { describe, expect, it } from 'vitest'

import { validateEndpoint, validateKey } from './endpoint'

describe('validateEndpoint', () => {
  it('lets an empty field through: it is the preset', () => {
    expect(validateEndpoint('rpc', '')).toBeNull()
    expect(validateEndpoint('explorer', '   ')).toBeNull()
  })

  it('accepts https, and http only to localhost for a node', () => {
    expect(validateEndpoint('rpc', 'https://node.example.org:8545/path?x=1')).toBeNull()
    expect(validateEndpoint('rpc', 'http://localhost:8545')).toBeNull()
    expect(validateEndpoint('rpc', 'http://127.0.0.1:8545')).toBeNull()
    expect(validateEndpoint('blockscout', 'http://localhost:4000')).toBeNull()
  })

  it('refuses plain http elsewhere, other schemes and credentials', () => {
    for (const bad of [
      'http://node.example.org',
      'http://localhost.example.org',
      'ftp://x.org',
      'javascript:alert(1)',
      'node.example.org',
      'https://u:p@node.example.org',
    ]) {
      expect(validateEndpoint('rpc', bad), bad).not.toBeNull()
    }
  })

  it('keeps a query out of Blockscout and the explorer, and a fragment out of all', () => {
    expect(validateEndpoint('blockscout', 'https://scout.example.org/?a=1')).not.toBeNull()
    expect(validateEndpoint('explorer', 'https://explorer.example.org/?a=1')).not.toBeNull()
    expect(validateEndpoint('rpc', 'https://node.example.org/#x')).not.toBeNull()
  })

  it('holds the explorer to https with a dotted host, even for localhost', () => {
    expect(validateEndpoint('explorer', 'http://localhost:3000')).not.toBeNull()
    expect(validateEndpoint('explorer', 'https://localhost')).not.toBeNull()
    expect(validateEndpoint('explorer', 'https://explorer.example.org')).toBeNull()
  })

  it('refuses whitespace', () => {
    expect(validateEndpoint('rpc', 'https://node .example.org')).not.toBeNull()
  })
})

describe('validateKey', () => {
  it('is optional', () => {
    expect(validateKey('')).toBeNull()
  })

  it('wants visible characters a header can carry', () => {
    expect(validateKey('proapi_abc123')).toBeNull()
    expect(validateKey('has space')).not.toBeNull()
    expect(validateKey('ключ')).not.toBeNull()
    expect(validateKey('a'.repeat(257))).not.toBeNull()
  })
})
