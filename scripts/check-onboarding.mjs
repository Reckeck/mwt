// The onboarding gate (AGENTS.md rule 12, skill `onboarding`): every screen
// has a tour, every tour step points at an element that exists, every tour,
// «Что нового» and learning-centre text is there in both locales.
//
// Static: reads the sources, runs nothing. Node strips the types of the two
// small TypeScript modules it imports. Exit code 1 on an error; unused
// `data-tour` targets are only reported.
import { readdirSync, readFileSync } from 'node:fs'
import { join, relative } from 'node:path'

const root = new URL('..', import.meta.url).pathname
const { TOURS, TOURLESS_ROUTES } = await import(join(root, 'src/shared/lib/tour/tours.ts'))
const { WHATS_NEW } = await import(join(root, 'src/shared/config/whats-new.ts'))

const errors = []
const warnings = []

function walk(dir, ext, out = []) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name)
    if (entry.isDirectory()) walk(path, ext, out)
    else if (entry.name.endsWith(ext)) out.push(path)
  }
  return out
}

// Routes by name.
const routes = new Set()
for (const file of walk(join(root, 'src/app/providers/router'), '.ts')) {
  for (const match of readFileSync(file, 'utf8').matchAll(/name: '([^']+)'/g)) routes.add(match[1])
}

// Static `data-tour="…"` targets in the components.
const targets = new Map()
for (const file of walk(join(root, 'src'), '.vue')) {
  for (const match of readFileSync(file, 'utf8').matchAll(/data-tour="([a-z0-9-]+)"/g)) {
    targets.set(match[1], relative(root, file))
  }
}

const locales = Object.fromEntries(
  ['ru', 'en'].map((lang) => [
    lang,
    JSON.parse(readFileSync(join(root, `src/shared/config/i18n/locales/${lang}.json`), 'utf8')),
  ]),
)
function needKey(key, where) {
  for (const [lang, messages] of Object.entries(locales)) {
    if (!(key in messages)) errors.push(`${where}: i18n key "${key}" is missing in ${lang}.json`)
  }
}

for (const route of routes) {
  if (!(route in TOURS) && !(route in TOURLESS_ROUTES)) {
    errors.push(`route "${route}" has no tour: add one to TOURS or a reason to TOURLESS_ROUTES`)
  }
}

const used = new Set()
for (const [route, steps] of Object.entries(TOURS)) {
  if (!routes.has(route)) errors.push(`tour "${route}" is for no route`)
  if (!steps.length) errors.push(`tour "${route}" has no steps`)
  needKey(`tourName.${route}`, `tour "${route}"`)
  for (const step of steps) {
    used.add(step.target)
    if (!targets.has(step.target)) {
      errors.push(`tour "${route}": no element has data-tour="${step.target}"`)
    }
    needKey(`tour.${step.key}.title`, `tour "${route}"`)
    needKey(`tour.${step.key}.body`, `tour "${route}"`)
  }
}

for (const entry of WHATS_NEW) {
  needKey(`whatsNew.${entry.id}.title`, `whats-new "${entry.id}"`)
  for (const point of entry.points) {
    needKey(`whatsNew.${entry.id}.${point.key}`, `whats-new "${entry.id}"`)
    // `settings:<tab>` opens a settings tab rather than a screen with a tour.
    if (point.route && !point.route.startsWith('settings:') && !(point.route in TOURS)) {
      errors.push(`whats-new "${entry.id}.${point.key}": route "${point.route}" has no tour to show`)
    }
  }
}

// The two locales hold the same keys (AGENTS.md: the key sets must match).
const ru = new Set(Object.keys(locales.ru))
const en = new Set(Object.keys(locales.en))
for (const key of ru) if (!en.has(key)) errors.push(`i18n key "${key}" is in ru.json only`)
for (const key of en) if (!ru.has(key)) errors.push(`i18n key "${key}" is in en.json only`)

for (const [target, file] of targets) {
  if (!used.has(target)) warnings.push(`data-tour="${target}" (${file}) is in no tour`)
}

for (const line of warnings) console.warn(`warning: ${line}`)
for (const line of errors) console.error(`error: ${line}`)
console.log(
  `check-onboarding: ${routes.size} routes, ${Object.keys(TOURS).length} tours, ${errors.length} errors, ${warnings.length} warnings`,
)
process.exit(errors.length ? 1 : 0)
