// The crypto operations screen is read-only (spec D14, E10): a crypto record is kept by hand
// elsewhere, here it is only listed. Static: reads the sources of the screen's two slices.
//
// - the only backend call is `crypto_list_operations`; no `crypto_create_`, `crypto_delete_`,
//   `crypto_remove_`, `crypto_add_` command;
// - nothing leaves the app: no `fetch`, `XMLHttpRequest`, `window.open`, `href` or `<img>`.
//
// `entities/crypto-tx/model/readonly-lists.ts` holds the same rule for the whole entity; folding
// this guard into it is a follow-up once both streams are merged.
import { readFileSync } from 'node:fs'
import { join, relative } from 'node:path'

import { describe, it } from 'vitest'

import { expectNone, ROOT } from './lib/report'
import { lineOf, walk } from './lib/walk'

const SLICES = ['src/widgets/crypto-operations', 'src/pages/crypto-operations']
const ALLOWED_CALL = 'crypto_list_operations'

const CALL = /\bcall(?:<[^>(]*>)?\(\s*['"`]([^'"`]+)['"`]/g
const WRITES = /\bcrypto_(?:create|delete|remove|add)_\w*/g
const LEAVES = /\bfetch\(|\bXMLHttpRequest\b|\bwindow\.open\b|\bhref\b|<img\b/g

function scan(): string[] {
  const found: string[] = []
  for (const slice of SLICES) {
    for (const file of walk(join(ROOT, slice), /\.(?:vue|ts)$/)) {
      if (/\.test\.ts$/.test(file)) continue
      const text = readFileSync(file, 'utf8')
      const add = (index: number, message: string) =>
        found.push(`${relative(ROOT, file)}:${lineOf(text, index)}: ${message}`)
      for (const match of text.matchAll(CALL))
        if (match[1] !== ALLOWED_CALL)
          add(match.index, `call('${match[1]}'): only ${ALLOWED_CALL} is called here (E10)`)
      for (const match of text.matchAll(WRITES))
        add(match.index, `${match[0]}: the operations screen never writes (E10)`)
      for (const match of text.matchAll(LEAVES)) add(match.index, `${match[0]}: nothing leaves the app from here (E10)`)
    }
  }
  return found
}

describe('crypto operations are read-only (E10)', () => {
  it('lists through crypto_list_operations only and reaches nowhere else', () => {
    expectNone(scan())
  })

  it('has slices to guard', () => {
    const files = SLICES.flatMap((slice) => walk(join(ROOT, slice), /\.(?:vue|ts)$/))
    if (files.length === 0) throw new Error('error: no crypto operations sources found: update SLICES')
  })
})
