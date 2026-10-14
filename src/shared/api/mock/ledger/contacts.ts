/** Debts and turnover per contact. */

/** Personal debts per contact and asset, like `contact_balances` in Rust. */
export function mockContactBalances(txs: Transaction[]): ContactBalance[] {
  const out = new Map<string, ContactBalance>()
  for (const tx of txs) {
    if (!('transfer' in tx) || !tx.transfer.debt) continue
    const { from, to, debt, amount } = tx.transfer
    const contact = 'contact' in from ? from.contact.contact_id : 'contact' in to ? to.contact.contact_id : null
    if (!contact) continue
    const key = `${contact}:${tx.transfer.asset.symbol}`
    const row = out.get(key) ?? { contact_id: contact, asset: tx.transfer.asset, receivable: '0', payable: '0' }
    const delta = Number(amount)
    if (debt === 'lend') row.receivable = String(Number(row.receivable) + delta)
    if (debt === 'collect') row.receivable = String(Number(row.receivable) - delta)
    if (debt === 'borrow') row.payable = String(Number(row.payable) + delta)
    if (debt === 'repay') row.payable = String(Number(row.payable) - delta)
    out.set(key, row)
  }
  return [...out.values()].filter((row) => Number(row.receivable) || Number(row.payable))
}

/** Sent and received per asset with one contact, from principal legs. */
export function mockTurnover(
  txs: Transaction[],
  contactId: string,
  accountId: string | null,
  legsOf: (tx: Transaction) => Array<{ asset: AssetRef; amount: string; role: LegRole }>,
): ContactTurnover {
  const rows = new Map<string, TurnoverRow>()
  let count = 0
  for (const tx of txs) {
    const kind = Object.keys(tx)[0] as TxKind
    const record = (tx as unknown as Record<TxKind, { meta: TxMeta; from?: AccountRef; to?: AccountRef }>)[kind]
    const sides = [record.from, record.to].filter((side): side is AccountRef =>
      Boolean(side && typeof side === 'object'),
    )
    const named = sides.some(
      (side) =>
        'contact' in side &&
        side.contact.contact_id === contactId &&
        (!accountId || side.contact.account_id === accountId),
    )
    if (!named && (accountId || record.meta.contact_id !== contactId)) continue
    count += 1
    for (const leg of legsOf(tx).filter((entry) => entry.role === 'principal')) {
      const row = rows.get(leg.asset.symbol) ?? { asset: leg.asset, sent: '0', received: '0' }
      if (leg.amount.startsWith('-')) row.sent = String(Number(row.sent) + Math.abs(Number(leg.amount)))
      else row.received = String(Number(row.received) + Number(leg.amount))
      rows.set(leg.asset.symbol, row)
    }
  }
  return {
    contact_id: contactId,
    account_id: accountId,
    base_currency: 'usd',
    rows: [...rows.values()],
    sent_base: '0',
    received_base: '0',
    tx_count: count,
  }
}
