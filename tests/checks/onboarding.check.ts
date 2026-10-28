// The onboarding gate (AGENTS.md rule 12, skill `onboarding`): every screen has a tour, every tour
// step points at an element that exists, every tour, «Что нового» and learning-centre text is there
// in both locales. Static: reads the sources, runs nothing. Unused `data-tour` targets are only reported.
import { readFileSync } from 'node:fs'
import { join, relative } from 'node:path'

import { describe, it } from 'vitest'

import { TOURLESS_ROUTES, TOURS } from '@/shared/lib/tour/tours'
import { WHATS_NEW } from '@/shared/config/whats-new'

import { expectNone, ROOT } from './lib/report'
import { routeNames } from './lib/routes'
import { walk } from './lib/walk'

const routes = routeNames(ROOT)

// Static `data-tour="…"` targets in the components.
const targets = new Map<string, string>()
for (const file of walk(join(ROOT, 'src'), '.vue')) {
  for (const match of readFileSync(file, 'utf8').matchAll(/data-tour="([a-z0-9-]+)"/g)) {
    targets.set(match[1], relative(ROOT, file))
  }
}

const locales: Record<string, Record<string, string>> = Object.fromEntries(
  ['ru', 'en'].map((lang) => [
    lang,
    JSON.parse(readFileSync(join(ROOT, `src/shared/config/i18n/locales/${lang}.json`), 'utf8')),
  ]),
)

function missingKey(key: string, where: string): string[] {
  return Object.entries(locales)
    .filter(([, messages]) => !(key in messages))
    .map(([lang]) => `${where}: i18n key "${key}" is missing in ${lang}.json`)
}

describe('check-onboarding', () => {
  it('every route has a tour (or a reason for none)', () => {
    expectNone(
      [...routes]
        .filter((route) => !(route in TOURS) && !(route in TOURLESS_ROUTES))
        .map((route) => `route "${route}" has no tour: add one to TOURS or a reason to TOURLESS_ROUTES`),
    )
  })

  it('every tour is for a route, has steps, points at existing elements, has its texts', () => {
    const errors: string[] = []
    for (const [route, steps] of Object.entries(TOURS)) {
      if (!routes.has(route)) errors.push(`tour "${route}" is for no route`)
      if (!steps.length) errors.push(`tour "${route}" has no steps`)
      errors.push(...missingKey(`tourName.${route}`, `tour "${route}"`))
      for (const step of steps) {
        if (!targets.has(step.target)) errors.push(`tour "${route}": no element has data-tour="${step.target}"`)
        errors.push(...missingKey(`tour.${step.key}.title`, `tour "${route}"`))
        errors.push(...missingKey(`tour.${step.key}.body`, `tour "${route}"`))
      }
    }
    expectNone(errors)
  })

  it('«Что нового» entries have texts and point at routes with a tour', () => {
    const errors: string[] = []
    for (const entry of WHATS_NEW) {
      errors.push(...missingKey(`whatsNew.${entry.id}.title`, `whats-new "${entry.id}"`))
      for (const point of entry.points) {
        errors.push(...missingKey(`whatsNew.${entry.id}.${point.key}`, `whats-new "${entry.id}"`))
        // `settings:<group>[:<section>]` opens a settings group rather than a screen with a tour.
        if (point.route && !point.route.startsWith('settings:') && !(point.route in TOURS)) {
          errors.push(`whats-new "${entry.id}.${point.key}": route "${point.route}" has no tour to show`)
        }
      }
    }
    expectNone(errors)
  })

  it('the two locales hold the same keys', () => {
    const ru = new Set(Object.keys(locales.ru))
    const en = new Set(Object.keys(locales.en))
    expectNone([
      ...[...ru].filter((key) => !en.has(key)).map((key) => `i18n key "${key}" is in ru.json only`),
      ...[...en].filter((key) => !ru.has(key)).map((key) => `i18n key "${key}" is in en.json only`),
    ])
  })

  it('reports', () => {
    const used = new Set(Object.values(TOURS).flatMap((steps) => steps.map((step) => step.target)))
    for (const [target, file] of targets) {
      if (!used.has(target)) console.warn(`warning: data-tour="${target}" (${file}) is in no tour`)
    }
    console.log(`check-onboarding: ${routes.size} routes, ${Object.keys(TOURS).length} tours`)
  })
})
