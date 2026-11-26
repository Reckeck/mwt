import { describe, expect, it } from 'vitest'

import { router } from './index'
import { routePlaceholder } from './route-placeholder'

/** Screens already rebuilt; each W4 task adds its route here. */
const REBUILT = new Set([
  'unlock',
  'stores',
  'money-wealth',
  'money-investments',
  'crypto-positions',
  'money-plan',
  'money-month',
])

describe('route placeholder', () => {
  it('is the body of every named leaf route except the dashboard shell parent', () => {
    const leaves = router
      .getRoutes()
      .filter(
        (route) => route.name && !REBUILT.has(String(route.name)) && !route.redirect && route.children.length === 0,
      )
    expect(leaves.length).toBeGreaterThan(0)
    for (const route of leaves) {
      if (REBUILT.has(String(route.name)))
        expect(route.components?.default, String(route.name)).not.toBe(routePlaceholder)
      else expect(route.components?.default, String(route.name)).toBe(routePlaceholder)
    }
  })
})
