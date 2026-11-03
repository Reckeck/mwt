import { txContactIds, txIsInternal, txLegs, txMeta } from '@/shared/lib/transactions'
import { MOCK_PER_USD } from '../fx-rates'
import { mockAppSettings } from '../vault-data'
import { mockContactBalances } from './contacts'

/** `value` of `code` in `base` through the mock's per-dollar rates; `null` when either has none. */
function convert(value: number, code: string, base: string): number | null {
  const from = MOCK_PER_USD[code.toLowerCase()]
  const to = MOCK_PER_USD[base.toLowerCase()]
  return from && to ? (value / from) * to : null
}

/** One row per contact, like `contact_summaries` in Rust. `balances` are the debts to convert. */
export function mockContactSummaries(
  contacts: Contact[],
  txs: Transaction[],
  balances: ContactBalance[] = mockContactBalances(txs),
): ContactSummary[] {
  const base = mockAppSettings.base_currency
  const rows = new Map<string, ContactSummary>(
    contacts.map((contact) => [
      contact.id,
      {
        contact_id: contact.id,
        tx_count: 0,
        last_tx_at: null,
        last_amount: null,
        last_asset: null,
        receivable_base: '0',
        payable_base: '0',
        missing_rate: false,
      },
    ]),
  )
  for (const tx of txs) {
    if (txIsInternal(tx, (id) => id)) continue
    const stamp = txMeta(tx).timestamp
    for (const id of txContactIds(tx)) {
      const row = rows.get(id)
      if (!row) continue
      row.tx_count += 1
      if (row.last_tx_at && row.last_tx_at > stamp) continue
      const leg = txLegs(tx).find((entry) => entry.role === 'principal')
      row.last_tx_at = stamp
      row.last_amount = leg?.amount ?? null
      row.last_asset = leg?.asset.symbol ?? null
    }
  }
  for (const balance of balances) {
    const row = rows.get(balance.contact_id)
    if (!row) continue
    const sides = [
      ['receivable_base', Number(balance.receivable)],
      ['payable_base', Number(balance.payable)],
    ] as const
    for (const [field, value] of sides) {
      if (!value) continue
      const converted = convert(value, balance.asset.symbol, base)
      if (converted === null) row.missing_rate = true
      else row[field] = String(Number(row[field]) + converted)
    }
  }
  return [...rows.values()]
}
