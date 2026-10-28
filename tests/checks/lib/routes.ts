import { readFileSync } from 'node:fs'
import { join } from 'node:path'

import { walk } from './walk'

/** Route names declared in the UI router (`name: '…'`). */
export function routeNames(root: string): Set<string> {
  const routes = new Set<string>()
  for (const file of walk(join(root, 'src/app/providers/router'), '.ts')) {
    for (const match of readFileSync(file, 'utf8').matchAll(/name: '([^']+)'/g)) routes.add(match[1])
  }
  return routes
}
