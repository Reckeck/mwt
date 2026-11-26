import { describe, expect, it } from 'vitest'

import { router } from './index'
import { routePlaceholder } from './route-placeholder'

describe('route placeholder', () => {
  it('is the body of every named leaf route except the dashboard shell parent', () => {
    const leaves = router.getRoutes().filter((route) => route.name && !route.redirect && route.children.length === 0)
    expect(leaves.length).toBeGreaterThan(0)
    for (const route of leaves) expect(route.components?.default, String(route.name)).toBe(routePlaceholder)
  })
})
