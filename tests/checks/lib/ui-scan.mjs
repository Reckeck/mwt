// The UI rules (skill `ui-design-system`): screens are built from the shared components, not from
// raw markup that drifts screen by screen. Static: reads `src/**`, runs nothing. Used by
// tests/checks/ui.check.ts; the rule list and its explanations are in the header of that file.
import { readdirSync, readFileSync } from 'node:fs'
import { join, relative } from 'node:path'

import { blankComments, callBody } from '../rules/ui/lib.mjs'
import { BEHAVIOUR_RULES } from '../rules/ui/rules-behaviour.mjs'
import { CSS_RULES } from '../rules/ui/rules-css.mjs'
import { MARKUP_RULES } from '../rules/ui/rules-markup.mjs'
import { MONEY_RULES } from '../rules/ui/rules-money.mjs'

const MAX_LINES = { '.vue': 250, '.ts': 300 }
const SIZE_EXEMPT = /(?:\.gen\.d\.ts|\.d\.ts)$/

function walk(dir, out = []) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name)
    if (entry.isDirectory()) walk(path, out)
    else if (/\.(?:vue|ts|css)$/.test(entry.name)) out.push(path)
  }
  return out
}

const lineOf = (text, index) => text.slice(0, index).split('\n').length

const RULES = [
  {
    id: 'raw-button',
    vueOnly: true,
    find: (text) => [...text.matchAll(/<button\b/g)],
    say: '<button> by hand — use Button from @/shared/ui/button',
  },
  {
    id: 'raw-check',
    vueOnly: true,
    find: (text) => [...text.matchAll(/<input\b[^>]*\btype="(?:checkbox|radio)"/gs)],
    say: 'native checkbox/radio — use Checkbox or RadioGroup',
  },
  {
    id: 'raw-table',
    vueOnly: true,
    find: (text) => [...text.matchAll(/<table\b/g)],
    say: '<table> by hand — use DataTable (or the shadcn Table parts)',
  },
  {
    id: 'hand-empty',
    vueOnly: true,
    find: (text) => [...text.matchAll(/class="[^"]*\bborder-dashed\b[^"]*\bp-(?:6|8)\b[^"]*"/g)],
    say: 'a dashed box as an empty state — use EmptyState or SectionState',
  },
  {
    id: 'raw-color',
    vueOnly: false,
    find: (text) => [
      ...text.matchAll(/\b(?:text|bg|border|fill|stroke|ring)-(?:emerald|green|red|rose|amber|yellow|lime)-\d{2,3}\b/g),
      ...text.matchAll(/(?<=['"\s:(])#[0-9a-fA-F]{6}\b/g),
    ],
    say: 'a raw colour — use a token (text-success, text-destructive, text-warning, var(--…))',
  },
  {
    id: 'transition-all',
    vueOnly: false,
    find: (text) => [...text.matchAll(/\btransition-all\b/g)],
    say: 'transition-all — animate only the properties that change (transition-colors, -transform, -opacity)',
  },
  {
    id: 'to-fixed',
    vueOnly: true,
    find: (text) => [...text.matchAll(/\.toFixed\(/g)],
    say: '.toFixed() in a component — format with formatMoney/formatPct/formatQuantity',
  },
  {
    id: 'div-click',
    vueOnly: true,
    find: (text) => [...text.matchAll(/<div\b(?=[^>]*@click)(?![^>]*\brole=)[^>]*>/gs)],
    say: '<div @click> without a role — use Button, or give it a role and keyboard handling',
  },
  {
    // The name of a wallet, category or contact shown as text cannot be opened: it is an
    // <EntityLink :label="…">. Names inside i18n arguments and attributes are fine.
    id: 'entity-text',
    vueOnly: true,
    find: (text) => {
      const start = text.indexOf('<template>')
      if (start === -1) return []
      return [...text.slice(start).matchAll(/\{\{[^{}]*\b(?:storeName|categoryName|contactName)\(/g)].map((match) =>
        Object.assign(match, { index: match.index + start }),
      )
    },
    say: 'an entity name as plain text — show it with <EntityLink :target :label> so it opens its card',
  },
  {
    // The phone's browser bars and keyboard change the height: `dvh` follows them, `vh` does not.
    id: 'viewport-height',
    vueOnly: false,
    find: (text) => [...text.matchAll(/\b(?:min-|max-)?h-screen\b|(?<![dsl])\b\d+vh\b/g)],
    say: 'h-screen / vh — use dvh (or h-full inside the shell), see references/mobile.md',
  },
  {
    // Pinned to an edge with a plain offset: on a phone it lands under the status or gesture bar.
    id: 'fixed-edge',
    vueOnly: true,
    find: (text) =>
      [...text.matchAll(/class="([^"]*)"/g)].filter(
        (match) =>
          /(?:^|\s)fixed(?:\s|$)/.test(match[1]) &&
          /(?:^|\s)(?:top|bottom|inset|inset-y)-(?:[1-9]|\[)/.test(match[1]) &&
          !/safe/.test(match[1]),
      ),
    say: 'fixed at an edge without the safe-area offset — use *-safe / var(--safe-*)',
  },
  {
    // 14rem and wider without a breakpoint does not fit a 360px phone beside anything else.
    id: 'fixed-width',
    vueOnly: true,
    find: (text) =>
      [...text.matchAll(/(?<![\w:-])(?:min-)?w-(?:(?:5[6-9]|[6-9]\d|\d{3})\b|\[(\d+(?:\.\d+)?)(rem|px)\])/g)].filter(
        (match) => !match[1] || Number(match[1]) >= (match[2] === 'rem' ? 14 : 224),
      ),
    say: 'a wide fixed width without a breakpoint — prefix it (sm:w-56), cap it (min(…,100vw-2rem)) or let it flex',
  },
  {
    id: 'mutation-meta',
    vueOnly: false,
    find: (text) =>
      [...text.matchAll(/\buseMutation\s*(?:<[^>]*>)?\s*\(/g)].filter((match) => {
        const body = callBody(text, match.index + match[0].length - 1)
        // `meta` may be a const or hold nested braces (an `undo` arrow): look past it.
        const at = body.search(/\bmeta\s*[:,}]/)
        if (at === -1) return true
        const metaConst = body.slice(at).match(/^meta\s*:\s*([A-Za-z_]\w*)\s*[,}\n]/)
        if (metaConst) return !new RegExp(`\\b${metaConst[1]}\\s*=\\s*\\{[^;]*\\b(?:success|silent)\\s*:`, 's').test(text)
        return !/\b(?:success|silent)\s*:/.test(body.slice(at))
      }),
    say: 'useMutation without meta.success (or meta.silent with a reason) — every action reports its outcome',
  },
  ...BEHAVIOUR_RULES,
  ...MARKUP_RULES,
  ...MONEY_RULES,
]

export const SAY = Object.fromEntries([...RULES, ...CSS_RULES].map((rule) => [rule.id, rule.say]))
SAY['file-size'] = 'file too long — split it into model/ composables, lib/ helpers and ui/ parts'

export const RULE_IDS = [...Object.keys(SAY)]

/** Walk `src/` once: per `<rule> <file>` the count and the first hit, plus the repeated class strings. */
export function scanUi(root) {
  const counts = new Map()
  const firstHit = new Map()
  const classFiles = new Map()

  function note(rule, file, text, index) {
    const key = `${rule.id} ${file}`
    counts.set(key, (counts.get(key) ?? 0) + 1)
    if (!firstHit.has(key)) firstHit.set(key, `${file}:${lineOf(text, index)}`)
  }

  for (const path of walk(join(root, 'src'))) {
    const file = relative(root, path)
    const source = readFileSync(path, 'utf8')
    const text = blankComments(source)
    const ignored = new Set([...source.matchAll(/check-ui-ignore ([a-z-]+): \S/g)].map((match) => match[1]))
    if (file.endsWith('.css')) {
      for (const rule of CSS_RULES) {
        if (ignored.has(rule.id)) continue
        for (const match of rule.find(text, file)) note(rule, file, text, match.index)
      }
      continue
    }
    const isVue = file.endsWith('.vue')
    const isPrimitive = file.startsWith('src/shared/ui/')

    // The browser mock is data, not UI: its hex values are the colours of mock wallets.
    const isMock = file.startsWith('src/shared/api/mock/')
    for (const rule of RULES) {
      if (isPrimitive && !rule.primitives) continue
      if (isMock && rule.id === 'raw-color') continue
      if (ignored.has(rule.id)) continue
      if (rule.vueOnly && !isVue) continue
      if (rule.skip?.(file)) continue
      for (const match of rule.find(text, file, source)) note(rule, file, text, match.index)
    }
    if (isVue && !isPrimitive) {
      for (const match of text.matchAll(/\bclass="([^"]{40,})"/g)) {
        const key = match[1].trim().split(/\s+/).sort().join(' ')
        if (!classFiles.has(key)) classFiles.set(key, new Set())
        classFiles.get(key).add(file)
      }
    }

    const ext = isVue ? '.vue' : '.ts'
    if (!SIZE_EXEMPT.test(file)) {
      const lines = text.split('\n').length
      if (lines > MAX_LINES[ext]) {
        const key = `file-size ${file}`
        counts.set(key, lines)
        firstHit.set(key, `${file}:1`)
      }
    }
  }
  return { counts, firstHit, classFiles }
}

/** Errors past the baseline, per rule id, and the baseline entries that got better. */
export function judgeUi(scan, baseline) {
  const errors = []
  const improved = []
  for (const [key, count] of scan.counts) {
    const allowed = baseline[key] ?? 0
    // A known long file may still change while it waits for its split; a new one may not be long.
    if (key.startsWith('file-size ') && key in baseline) continue
    if (count > allowed) {
      const [rule] = key.split(' ')
      const unit = rule === 'file-size' ? `${count} lines` : `${count}×`
      const was = allowed ? ` (baseline ${allowed})` : ''
      errors.push({ rule, text: `${scan.firstHit.get(key)}: [${rule}] ${SAY[rule]} — ${unit}${was}` })
    }
  }
  for (const [key, allowed] of Object.entries(baseline)) {
    const count = scan.counts.get(key) ?? 0
    if (key.startsWith('file-size ') ? count === 0 : count < allowed) improved.push(key)
  }
  return { errors, improved }
}

/** The baseline after `--update`: an entry only shrinks (or appears on the very first run). */
export function nextUiBaseline(scan, baseline) {
  const next = {}
  for (const key of [...scan.counts.keys()].sort()) {
    const count = scan.counts.get(key)
    next[key] = key in baseline || Object.keys(baseline).length === 0 ? Math.min(count, baseline[key] ?? count) : count
  }
  return next
}

/** A class string of 40+ characters copied into 4+ files: a hint, not a failure. */
export const copiedClasses = (scan) => [...scan.classFiles].filter(([, files]) => files.size >= 4)
