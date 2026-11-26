import { describe, expect, it } from 'vitest'

import { groupByPurpose } from './group-by-purpose'

const node = (name: string, purpose: Purpose, base: string, extra: Partial<Store> = {}) =>
  ({
    store: { id: name, name, kind: 'bank', purpose, archived: false, ...extra },
    subtree_base: base,
    children: [],
  }) as unknown as StoreTreeNode

describe('groupByPurpose', () => {
  it('orders the groups, sums them and skips empty ones', () => {
    const groups = groupByPurpose([
      node('Broker', 'investment', '300'),
      node('Card', 'everyday', '100'),
      node('Cash', 'everyday', '50'),
    ])
    expect(groups.map((group) => group.purpose)).toEqual(['everyday', 'investment'])
    expect(groups[0]?.total).toBe(150)
  })

  it('leaves out archived accounts and bonus programmes', () => {
    const groups = groupByPurpose([
      node('Old', 'savings', '10', { archived: true }),
      node('Miles', 'everyday', '5', { kind: 'loyalty' }),
    ])
    expect(groups).toEqual([])
  })
})
