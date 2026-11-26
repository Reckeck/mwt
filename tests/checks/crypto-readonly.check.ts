// What the crypto engine drives only reads: it writes `crypto_events`, so no screen may create, edit or
// delete an operation or a position, or correct a balance by hand (decision P5, spec E9). The screens that
// show the module are listed here; this check keeps a write command or a fiat write surface from slipping in.
// Static: reads the sources, runs nothing.
import { readFileSync } from 'node:fs'
import { join, relative } from 'node:path'

import { describe, expect, it } from 'vitest'

import { CRYPTO_ALLOWED_CALLS, CRYPTO_MODULE_SLICES } from '../../src/entities/crypto-tx/model/readonly-lists'
import { expectNone, ROOT } from './lib/report'
import { walk } from './lib/walk'

const MODULE = CRYPTO_MODULE_SLICES.map((part) => join(ROOT, part))
const ALLOWED_CALLS: readonly string[] = CRYPTO_ALLOWED_CALLS

/** A fiat write surface: none of these belongs to a screen of the module. */
const FIAT_WRITE =
  /TransactionCreate|TransactionEdit|transaction-delete|TxRowMenu|StoreTransactions|StoreAdjust|store-adjust/

function sourcesOf(root: string): string[] {
  return [...walk(root, '.vue'), ...walk(root, '.ts')].filter((file) => !file.endsWith('.test.ts'))
}

describe('check-crypto-readonly', () => {
  it('the crypto screens ask the backend only for a read, a token verdict or market data', () => {
    const errors: string[] = []
    for (const root of MODULE) {
      for (const file of sourcesOf(root)) {
        const text = readFileSync(file, 'utf8')
        for (const match of text.matchAll(/call(?:<[^>]*>)?\(\s*'([a-z_]+)'/g)) {
          if (!ALLOWED_CALLS.includes(match[1])) {
            errors.push(
              `${relative(ROOT, file)}: calls \`${match[1]}\`, which is not a read or a verdict; the engine owns the operations and positions, a screen cannot add or drop one`,
            )
          }
        }
      }
    }
    expectNone(errors)
  })

  it('the crypto screens carry no form that would create, edit or delete an operation', () => {
    const errors: string[] = []
    for (const root of MODULE) {
      for (const file of sourcesOf(root)) {
        const found = FIAT_WRITE.exec(readFileSync(file, 'utf8'))
        if (found) {
          errors.push(
            `${relative(ROOT, file)}: brings in \`${found[0]}\`, a fiat write surface; the module is read-only`,
          )
        }
      }
    }
    expectNone(errors)
  })

  it('the positions page offers no add, edit or delete of a position', () => {
    const page = readFileSync(join(ROOT, 'src/pages/crypto-positions/ui/CryptoPositionsPage.vue'), 'utf8')
    expect(page).not.toMatch(/useMutation|\.mutate\(|addPosition|deletePosition|editPosition/)
  })
})
