// The onboarding gate (AGENTS.md rule 12, skill `onboarding`): every screen has a tour, every tour
// step points at an element that exists, every tour, `What's new` and learning-centre text is there
// in both locales. Static: reads the sources, runs nothing. Unused `data-tour` targets are only reported.
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, relative } from 'node:path'

import { describe, expect, it } from 'vitest'

import { duplicateLocaleKeys, mergeLocale } from '@/shared/config/i18n/parts'
import type { LocaleMessages, LocaleParts } from '@/shared/config/i18n/parts'
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

const readJson = (path: string): LocaleMessages => JSON.parse(readFileSync(path, 'utf8'))

/** Every language as the app sees it, main file and parts together, and the keys two files both define. */
function readLocales(dir: string) {
  const parts: LocaleParts = Object.fromEntries(
    readdirSync(join(dir, 'parts'))
      .filter((name) => name.endsWith('.json'))
      .map((name) => [name, readJson(join(dir, 'parts', name))]),
  )
  const bases = ['ru', 'en'].map((lang) => [lang, readJson(join(dir, `${lang}.json`))] as const)
  return {
    locales: Object.fromEntries(bases.map(([lang, base]) => [lang, mergeLocale(base, lang, parts)])),
    duplicates: bases.flatMap(([lang, base]) => duplicateLocaleKeys(base, lang, parts)),
  }
}

function differingKeys(locales: Record<string, LocaleMessages>): string[] {
  const ru = new Set(Object.keys(locales.ru))
  const en = new Set(Object.keys(locales.en))
  return [
    ...[...ru].filter((key) => !en.has(key)).map((key) => `i18n key "${key}" is in ru.json only`),
    ...[...en].filter((key) => !ru.has(key)).map((key) => `i18n key "${key}" is in en.json only`),
  ]
}

const { locales, duplicates } = readLocales(join(ROOT, 'src/shared/config/i18n/locales'))

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
    expectNone(differingKeys(locales))
  })

  it('no key is defined in two files of a language', () => {
    expectNone(duplicates)
  })

  it('a key in a part of one language only fails the comparison, a repeated key is found', () => {
    const dir = mkdtempSync(join(tmpdir(), 'check-onboarding-'))
    try {
      mkdirSync(join(dir, 'parts'))
      writeFileSync(join(dir, 'ru.json'), '{ "shared.key": "текст" }')
      writeFileSync(join(dir, 'en.json'), '{ "shared.key": "text" }')
      writeFileSync(join(dir, 'parts', 'demo.ru.json'), '{ "demo.only": "текст", "shared.key": "ещё раз" }')
      const parsed = readLocales(dir)
      expect(differingKeys(parsed.locales)).toEqual(['i18n key "demo.only" is in ru.json only'])
      expect(parsed.duplicates).toEqual(['i18n key "shared.key" is in both ru.json and demo.ru.json'])
    } finally {
      rmSync(dir, { recursive: true })
    }
  })

  it('reports', () => {
    const used = new Set(Object.values(TOURS).flatMap((steps) => steps.map((step) => step.target)))
    for (const [target, file] of targets) {
      if (!used.has(target)) console.warn(`warning: data-tour="${target}" (${file}) is in no tour`)
    }
    console.log(`check-onboarding: ${routes.size} routes, ${Object.keys(TOURS).length} tours`)
  })
})
