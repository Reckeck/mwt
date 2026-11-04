import { describe, expect, it } from 'vitest'

import { categoryKeyLabelOrDeleted } from './categories'
import { nameOrDeleted } from './deleted-record'

const t = (key: string) =>
  key === 'record.deleted' ? 'Deleted record' : key === 'categories.default.food' ? 'Food' : key

describe('deleted record placeholder', () => {
  it('shows the placeholder for a store or contact that no longer resolves', () => {
    expect(nameOrDeleted(undefined, true, 'store-1', t)).toBe('Deleted record')
  })

  it('keeps the id while the list is loading and the name when it resolves', () => {
    expect(nameOrDeleted(undefined, false, 'store-1', t)).toBe('store-1')
    expect(nameOrDeleted('Cash', true, 'store-1', t)).toBe('Cash')
  })

  it('shows the placeholder for a category that is neither listed nor built in', () => {
    expect(categoryKeyLabelOrDeleted('gone', [], t)).toBe('Deleted record')
    expect(categoryKeyLabelOrDeleted('food', [], t)).toBe('Food')
    expect(categoryKeyLabelOrDeleted('gone', undefined, t)).toBe('gone')
    expect(categoryKeyLabelOrDeleted('mine', [{ key: 'mine', name: 'Mine' } as Category], t)).toBe('Mine')
  })
})
