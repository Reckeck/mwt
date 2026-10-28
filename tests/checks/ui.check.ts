// The UI gate (skill `ui-design-system`): screens are built from the shared components, not from
// raw markup that drifts screen by screen. Static: reads `src/**`, runs nothing.
//
// A rule is a `describe`; a new violation is a failed test with `file:line: [rule] text`.
// Rules (outside `src/shared/ui`, where the primitives themselves live): raw-button, raw-check,
// raw-table, hand-empty, raw-color, transition-all, to-fixed, div-click, money-text / money-lib,
// entity-text, viewport-height, fixed-edge, fixed-width, mutation-meta, file-size, and the stability
// rules of rule 17 (tests/checks/rules/ui: listeners and timers without cleanup, hover-mounted
// content, swapped controls, queries that blank on a key change, keyed RouterView, blur, hover-only
// actions, fixed grid tracks, the z/size scales, raw fields, focus rings, the stylesheet).
// `dup-classes` only warns: a long class string copied into 4+ files.
//
// Debts that predate a rule live in `scripts/check-ui.baseline.json` as `{ "<rule> <file>": count }`.
// A file may not get worse than its entry; when it gets better, `mise run check-ui -- --update` writes
// the smaller number so it cannot slide back. New files have no entry, so they must be clean.
//
// A file whose values are data, not UI, says so once, with the reason:
//   // check-ui-ignore raw-color: store colours are saved in the vault as hex
// A `file-size` entry only says "known to be long": the file may change until it is split.
import { readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

import { describe, it } from 'vitest'

import { copiedClasses, judgeUi, nextUiBaseline, RULE_IDS, scanUi } from './lib/ui-scan.mjs'
import { expectNone, ROOT, UPDATE } from './lib/report'

const BASELINE = join(ROOT, 'scripts/check-ui.baseline.json')

function readBaseline(): Record<string, number> {
  try {
    return JSON.parse(readFileSync(BASELINE, 'utf8'))
  } catch {
    return {}
  }
}

const scan = scanUi(ROOT)
const baseline = readBaseline()
const { errors, improved } = judgeUi(scan, baseline)

describe('check-ui', () => {
  if (UPDATE) {
    it('writes the baseline', () => {
      const next = nextUiBaseline(scan, baseline)
      writeFileSync(BASELINE, `${JSON.stringify(next, null, 2)}\n`)
      console.log(`check-ui: baseline written, ${Object.keys(next).length} entries`)
    })
    return
  }

  describe.each(RULE_IDS)('%s', (rule) => {
    it('has no new violations', () => {
      expectNone(errors.filter((error) => error.rule === rule).map((error) => error.text))
    })
  })

  it('reports', () => {
    // Repeated markup is a hint, not a failure: a shared component may be due.
    for (const [classes, files] of copiedClasses(scan)) {
      console.warn(`warn: [dup-classes] "${classes.slice(0, 70)}…" in ${files.size} files — a shared component?`)
    }
    if (improved.length) {
      console.log(
        `check-ui: ${improved.length} baseline entries improved — run \`mise run check-ui -- --update\` to lock them in`,
      )
    }
    console.log(`check-ui: ${errors.length} errors, ${Object.values(baseline).length} known debts in the baseline`)
  })
})
