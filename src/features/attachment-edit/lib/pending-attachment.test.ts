import { describe, expect, it } from 'vitest'

import { looksLikeLink, readPendingFile } from './pending-attachment'

describe('looksLikeLink', () => {
  it('takes an address with or without the scheme', () => {
    expect(looksLikeLink('https://example.com/a')).toBe(true)
    expect(looksLikeLink('t.me/chat')).toBe(true)
  })
  it('refuses plain words', () => {
    expect(looksLikeLink('receipt')).toBe(false)
    expect(looksLikeLink('two words.com')).toBe(false)
  })
})

describe('readPendingFile', () => {
  it('keeps the name and the type and encodes the body', async () => {
    const item = await readPendingFile(new File(['hi'], 'a.txt', { type: 'text/plain' }))
    expect(item).toEqual({ kind: 'file', name: 'a.txt', mime: 'text/plain', dataBase64: 'aGk=' })
  })
})
