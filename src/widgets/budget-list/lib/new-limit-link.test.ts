import { describe, expect, it } from 'vitest'

import { newLimitRequest, withoutNewLimit } from './new-limit-link'

describe('new limit link', () => {
  it('reads the category of the request', () => {
    expect(newLimitRequest({ new: 'limit', category: 'food' })).toEqual({ category: 'food' })
  })
  it('allows a request without a category', () => {
    expect(newLimitRequest({ new: 'limit' })).toEqual({ category: '' })
  })
  it('ignores other queries', () => {
    expect(newLimitRequest({ new: 'goal' })).toBeNull()
    expect(newLimitRequest({})).toBeNull()
  })
  it('drops only its own keys', () => {
    expect(withoutNewLimit({ new: 'limit', category: 'food', open: 'goal:g1' })).toEqual({ open: 'goal:g1' })
  })
})
