import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'

import { moneyRoutes } from './money.routes'

/** The route names of the budget entry in `NAV_SECTIONS` (read as text: the app layer may not import a page's internals). */
function budgetSectionRoutes(): string[] {
  const source = readFileSync(join(process.cwd(), 'src/pages/app-shell/lib/nav-sections.ts'), 'utf8')
  const budget = source.match(/id: 'budget',[\s\S]*?routes: \[([^\]]*)\]/)
  return [...(budget?.[1] ?? '').matchAll(/'([^']+)'/g)].map((match) => match[1])
}

describe('money routes', () => {
  it('wrap exactly the budget section routes in the MoneySection frame, in its order', () => {
    const frame = moneyRoutes.children?.find((child) => child.children?.length)
    const tabs = (frame?.children ?? []).map((child) => child.name)
    expect(tabs).toEqual(budgetSectionRoutes())
    expect(tabs).toHaveLength(6)
  })

  it('leaves the capital and people routes outside the frame', () => {
    const outside = (moneyRoutes.children ?? []).filter((child) => !child.children).map((child) => child.name)
    expect(outside).toEqual(['money-wealth', 'money-investments', 'money-together', 'money-together-event'])
  })
})
