import { readdirSync, readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'

import { NAV_SECTIONS, PROFILE_MENU_ROUTES, sectionOfRoute } from './nav-sections'

const routerDir = join(process.cwd(), 'src/app/providers/router')

function namedRoutes(): string[] {
  const names = new Set<string>()
  for (const file of readdirSync(routerDir).filter((entry) => entry.endsWith('.ts') && !entry.endsWith('.d.ts'))) {
    for (const match of readFileSync(join(routerDir, file), 'utf8').matchAll(/^\s*name: '([^']+)'/gm)) {
      names.add(match[1])
    }
  }
  return [...names]
}

describe('nav sections', () => {
  it('lists the five sections', () => {
    expect(NAV_SECTIONS.map((section) => section.id)).toEqual(['home', 'operations', 'budget', 'people', 'capital'])
  })

  it('maps every named router route to exactly one section or the profile menu', () => {
    const names = namedRoutes()
    expect(names.length).toBeGreaterThan(20)
    for (const name of names) {
      const owners = NAV_SECTIONS.filter((section) => section.routes.includes(name)).length
      const inProfile = PROFILE_MENU_ROUTES.includes(name) ? 1 : 0
      expect({ name, owners: owners + inProfile }).toEqual({ name, owners: 1 })
    }
  })

  it('lists no route the router does not have', () => {
    const names = new Set(namedRoutes())
    const listed = [...NAV_SECTIONS.flatMap((section) => section.routes), ...PROFILE_MENU_ROUTES]
    expect(listed.filter((name) => !names.has(name))).toEqual([])
  })

  it('opens a route of its own section from the root', () => {
    for (const section of NAV_SECTIONS) expect(section.routes).toContain(section.root)
    expect(sectionOfRoute('money-together-event')?.id).toBe('people')
  })
})
