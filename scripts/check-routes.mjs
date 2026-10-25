// Routes the backend links to: every name in `crates/core/src/routes.rs` must be
// a route of the UI router (`src/app/providers/router`), and no Rust source may
// hold a route name as a bare string. Static: reads the sources, runs nothing.
import { readdirSync, readFileSync } from 'node:fs'
import { join } from 'node:path'

const root = new URL('..', import.meta.url).pathname

function walk(dir, ext, out = []) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name)
    if (entry.isDirectory()) walk(path, ext, out)
    else if (entry.name.endsWith(ext)) out.push(path)
  }
  return out
}

const routes = new Set()
for (const file of walk(join(root, 'src/app/providers/router'), '.ts')) {
  for (const match of readFileSync(file, 'utf8').matchAll(/name: '([^']+)'/g)) routes.add(match[1])
}

const errors = []
const source = readFileSync(join(root, 'crates/core/src/routes.rs'), 'utf8')
const listed = [...source.matchAll(/^\s+[A-Z_]+ = "([^"]+)",$/gm)].map((match) => match[1])
if (listed.length === 0) errors.push('crates/core/src/routes.rs: no route names found')
for (const name of listed) {
  if (!routes.has(name)) errors.push(`routes.rs: "${name}" is not a route in src/app/providers/router`)
}

// A link written as a bare `"<name>"` next to `Some(` or `link:` bypasses the list.
const bare = /(?:Some\(|link: Some\()"([a-z]+(?:-[a-z]+)*)"/g
for (const file of walk(join(root, 'crates'), '.rs')) {
  if (file.endsWith('routes.rs')) continue
  const text = readFileSync(file, 'utf8')
  for (const match of text.matchAll(bare)) {
    if (routes.has(match[1]) || /^(planning|analytics)-/.test(match[1])) {
      errors.push(`${file.slice(root.length)}: route "${match[1]}" as a string, use trade_core::routes`)
    }
  }
}

if (errors.length > 0) {
  console.error(errors.join('\n'))
  process.exit(1)
}
console.log(`check-routes: ${listed.length} backend routes exist in the router (${routes.size} routes)`)
