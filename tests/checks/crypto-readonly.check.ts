// A wallet the crypto engine drives (a connected address, an exchange read through its key) only
// reads: the engine writes `crypto_events`, so nothing on its page may create, edit or delete an
// operation or correct the balance by hand (decision P5, spec E9). The wallet page serves wallets of
// every kind, so the ban lives in a flag, and this check keeps it from being dropped later:
//
// * every file of the wallet page that can start an operation (the markers below) must know the
//   `engine` flag, unless it is listed here with the reason it is safe;
// * the places that decide whether the page is "empty" (which offers `Add a balance`) must exclude an
//   engine wallet.
// Static: reads the sources, runs nothing.
import { readFileSync } from 'node:fs'
import { join, relative } from 'node:path'

import { describe, expect, it } from 'vitest'

import { expectNone, ROOT } from './lib/report'
import { walk } from './lib/walk'

const PAGE = join(ROOT, 'src/pages/wallet')

/** The screens of the module's operations (spec E10): the list, the card, the row and its corrections. */
const MODULE = [
  'src/widgets/crypto-operations',
  'src/entities/crypto-tx',
  'src/features/crypto-tx-override',
  'src/pages/crypto-operations',
  'src/features/crypto-ref-picker',
].map((part) => join(ROOT, part))

/**
 * Everything these screens are allowed to ask the backend for. The module reads the chain and writes
 * only corrections, so a command that would add or drop an operation cannot appear here by accident:
 * it has to be added to this list first, and that is the review.
 */
const ALLOWED_CALLS = [
  'crypto_list_operations',
  'crypto_operation_detail',
  'crypto_set_override',
  'open_explorer',
  // The pointer at a fiat record (spec E13): it annotates an operation the engine owns, it never
  // creates or drops one, and it changes no amount.
  'crypto_link_candidates',
  'crypto_link_pairs',
  'crypto_link_operation',
  'crypto_unlink_operation',
  // Who an address dealt with (spec E15): computed from the events on each read, writes nothing.
  'crypto_relations',
]

/** What starts an operation or a manual balance correction from the wallet page. */
const MARKERS = [
  /wallet\.addOperation/,
  /walletActions\(/,
  /emit\('(?:operation|adjust|act|revalue)'/,
  /openQuickAdd\(/,
  /adjustment\.open/,
  /automation\.makeFromStore/,
]

/** Files with a marker that are safe without the flag, and why. */
const EXEMPT: Record<string, string> = {
  // Only ever rendered while `isEmpty`, which is false for an engine wallet (checked below).
  'src/pages/wallet/ui/wallet-empty/WalletEmpty.vue': 'drawn only when the page is not engine-driven',
  // Draws the list it is given; the header passes an empty one for an engine wallet.
  'src/pages/wallet/ui/wallet-actions/WalletActions.vue': 'the header passes no actions for an engine wallet',
  // Pure data: the buttons a kind allows; the header empties the list for an engine wallet.
  'src/pages/wallet/lib/wallet-actions.ts': 'the header returns no actions for an engine wallet',
}

describe('check-crypto-readonly', () => {
  it('every file of the wallet page that can start an operation knows the engine flag', () => {
    const errors: string[] = []
    for (const file of [...walk(PAGE, '.vue'), ...walk(PAGE, '.ts')]) {
      const path = relative(ROOT, file)
      if (path in EXEMPT || path.endsWith('.test.ts')) continue
      const text = readFileSync(file, 'utf8')
      const marker = MARKERS.find((pattern) => pattern.test(text))
      if (marker && !/\bengine\b/.test(text)) {
        errors.push(
          `${path}: can start an operation (${marker}) but never checks \`engine\`; a wallet driven by the crypto engine only reads`,
        )
      }
    }
    expectNone(errors)
  })

  it('the header offers no operation buttons and the gear no manual actions to an engine wallet', () => {
    const header = readFileSync(join(PAGE, 'ui/wallet-header/WalletHeader.vue'), 'utf8')
    expect(header, 'WalletHeader must empty its actions for an engine wallet').toMatch(/props\.engine\s*\?\s*\[\]/)
    const gear = readFileSync(join(PAGE, 'ui/wallet-gear-menu/WalletGearMenu.vue'), 'utf8')
    for (const value of ['adjust', 'automation', 'sub-account']) {
      const line = gear.split('\n').find((row) => row.includes(`value="${value}"`)) ?? ''
      expect(line, `the gear item "${value}" must be hidden for an engine wallet`).toMatch(/manual|props\.engine/)
    }
  })

  it('the operations screens ask the backend only for a read or a correction', () => {
    const errors: string[] = []
    for (const root of MODULE) {
      for (const file of [...walk(root, '.vue'), ...walk(root, '.ts')]) {
        const path = relative(ROOT, file)
        if (path.endsWith('.test.ts')) continue
        const text = readFileSync(file, 'utf8')
        for (const match of text.matchAll(/call(?:<[^>]*>)?\(\s*'([a-z_]+)'/g)) {
          const name = match[1]
          if (!ALLOWED_CALLS.includes(name)) {
            errors.push(
              `${path}: calls \`${name}\`, which is not one of the module's reads or corrections; the engine owns the operations, a screen cannot add or drop one`,
            )
          }
        }
      }
    }
    expectNone(errors)
  })

  it('the operations screens carry no form that would create, edit or delete an operation', () => {
    const errors: string[] = []
    for (const root of MODULE) {
      for (const file of [...walk(root, '.vue'), ...walk(root, '.ts')]) {
        const path = relative(ROOT, file)
        if (path.endsWith('.test.ts')) continue
        const text = readFileSync(file, 'utf8')
        const found =
          /TransactionCreate|TransactionEdit|transaction-delete|TxRowMenu|StoreTransactions|StoreAdjust|store-adjust/.exec(
            text,
          )
        if (found) {
          errors.push(
            `${path}: brings in \`${found[0]}\`, a fiat write surface; an operation of the module is read-only`,
          )
        }
      }
    }
    expectNone(errors)
  })

  it('the page is never "empty" for an engine wallet, so it never offers `Add a balance`', () => {
    const page = readFileSync(join(PAGE, 'Wallet.vue'), 'utf8')
    const empty = page.slice(page.indexOf('const isEmpty'), page.indexOf('function openWallet'))
    expect(empty, 'Wallet.vue: isEmpty must exclude an engine wallet').toMatch(/!engine\.value/)
    const positions = readFileSync(join(PAGE, 'ui/wallet-positions/WalletPositions.vue'), 'utf8')
    expect(positions, 'WalletPositions: the engine branch must come before the `Add operation` empty state').toMatch(
      /v-if="props\.positions\.length === 0 && props\.engine"[\s\S]*v-else-if="props\.positions\.length === 0"/,
    )
  })
})
