import { describe, expect, it } from 'vitest'

import { buildOverride } from './build-override'

const row = (patch: Partial<CryptoOpRow> = {}) =>
  ({ id: 'op-1', categories: ['Food'], comment: null, contact_id: null, ...patch }) as CryptoOpRow
const fields = (patch = {}) => ({ category: 'Food', description: '', contact: '', ...patch })

describe('buildOverride', () => {
  it('sends nothing for a form left as it was', () => {
    expect(buildOverride(row(), fields(), false)).toEqual({ event_id: 'op-1', same_contract: false, clear: [] })
  })

  it('does not pin a category or a contact that was only shown', () => {
    const input = buildOverride(row({ contact_id: 'c-1' }), fields({ contact: 'c-1' }), false)
    expect(input.categories).toBeUndefined()
    expect(input.contact_id).toBeUndefined()
  })

  it('sends a changed category, description and contact', () => {
    const input = buildOverride(row(), fields({ category: 'Rent', description: ' note ', contact: 'c-2' }), false)
    expect(input).toMatchObject({ categories: ['Rent'], comment: 'note', contact_id: 'c-2' })
  })

  it('clears what the user emptied', () => {
    const input = buildOverride(row({ comment: 'old', contact_id: 'c-1' }), fields({ category: '' }), false)
    expect(input.clear).toEqual(['categories', 'comment', 'contact'])
  })

  it('sends the category and the contact when it is applied to the whole contract', () => {
    const input = buildOverride(row({ contact_id: 'c-1' }), fields({ contact: 'c-1' }), true)
    expect(input).toMatchObject({ same_contract: true, categories: ['Food'], contact_id: 'c-1' })
  })
})
