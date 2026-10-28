// Routes the backend links to: every name in `crates/core/src/routes.rs` must be a route of the UI
// router (`src/app/providers/router`), and no Rust source may hold a route name as a bare string.
// Static: reads the sources, runs nothing.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'

import { describe, it } from 'vitest'

import { expectNone, ROOT } from './lib/report'
import { routeNames } from './lib/routes'
import { walk } from './lib/walk'

const routes = routeNames(ROOT)
const source = readFileSync(join(ROOT, 'crates/core/src/routes.rs'), 'utf8')
const listed = [...source.matchAll(/^\s+[A-Z_]+ = "([^"]+)",$/gm)].map((match) => match[1])

describe('check-routes', () => {
  it('routes.rs lists route names that exist in the router', () => {
    const errors: string[] = []
    if (listed.length === 0) errors.push('crates/core/src/routes.rs: no route names found')
    for (const name of listed) {
      if (!routes.has(name)) errors.push(`routes.rs: "${name}" is not a route in src/app/providers/router`)
    }
    expectNone(errors)
  })

  it('no Rust source holds a route name as a bare string', () => {
    // A link written as a bare `"<name>"` next to `Some(` or `link:` bypasses the list.
    const bare = /(?:Some\(|link: Some\()"([a-z]+(?:-[a-z]+)*)"/g
    const errors: string[] = []
    for (const file of walk(join(ROOT, 'crates'), '.rs')) {
      if (file.endsWith('routes.rs')) continue
      const text = readFileSync(file, 'utf8')
      for (const match of text.matchAll(bare)) {
        if (routes.has(match[1]) || /^(planning|analytics)-/.test(match[1])) {
          errors.push(`${file.slice(ROOT.length)}: route "${match[1]}" as a string, use trade_core::routes`)
        }
      }
    }
    expectNone(errors)
  })
})
