import { describe, expect, it } from 'vitest'

import { liveRoots, purposeGroup } from './purpose-group'

const node = (name: string, purpose: Purpose, base: string, extra: Partial<Store> = {}) =>
  ({
    store: { id: name, name, kind: 'bank', purpose, archived: false, ...extra },
    subtree_base: base,
    children: [],
  }) as unknown as StoreTreeNode

describe('liveRoots', () => {
  it('drops archived accounts and bonus programmes', () => {
    const roots = [
      node('Card', 'everyday', '1'),
      node('Old', 'savings', '1', { archived: true }),
      node('Miles', 'everyday', '1', { kind: 'loyalty' }),
    ]
    expect(liveRoots(roots).map((item) => item.store.name)).toEqual(['Card'])
  })
})

describe('purposeGroup', () => {
  it('sums the live accounts of one purpose', () => {
    const group = purposeGroup(
      [
        node('A', 'investment', '300'),
        node('B', 'investment', '20'),
        node('C', 'everyday', '5'),
        node('D', 'investment', '9', { archived: true }),
      ],
      'investment',
    )
    expect(group.nodes.map((item) => item.store.name)).toEqual(['A', 'B'])
    expect(group.total).toBe(320)
  })
})
