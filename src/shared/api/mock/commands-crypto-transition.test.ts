import { describe, expect, it } from 'vitest'

import { cryptoTransitionCommands } from './commands-crypto-transition'
import { mockStores } from './demo'
import { seedMock } from './scenario-seed'

const call = <T>(cmd: 'crypto_transition_plan' | 'crypto_archive_manual_wallets') =>
  cryptoTransitionCommands[cmd]({ cmd, args: undefined, body: undefined }) as Promise<T>

// The demo history is seeded on the first command, as it is in the browser.
seedMock()

describe('the crypto transition mock', () => {
  it('retires the hand-kept wallet, leaves the exchange the module reads, and runs once', async () => {
    const plan = await call<CryptoTransitionPlan>('crypto_transition_plan')
    const names = plan.wallets.map((wallet) => wallet.name)

    expect(names).toContain('Ledger')
    expect(names).not.toContain('Bybit')
    expect(names).not.toContain('Futures')
    expect(plan.leaving_base).toBe('412300')

    const report = await call<CryptoArchiveReport>('crypto_archive_manual_wallets')
    expect(report.changed).toBe(true)
    expect(report.wallets.map((wallet) => wallet.name)).toEqual(names)
    // Archived, not deleted.
    expect(mockStores.filter((store) => store.name === 'Ledger' && store.archived)).toHaveLength(1)

    const again = await call<CryptoArchiveReport>('crypto_archive_manual_wallets')
    expect(again.changed).toBe(false)
    expect(again.wallets).toEqual([])
    expect((await call<CryptoTransitionPlan>('crypto_transition_plan')).wallets).toEqual([])
  })
})
