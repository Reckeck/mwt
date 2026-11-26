import type { QuarantineRow } from '../crypto-token.types'

/**
 * The quarantine of one account: what it holds of each token that is still quarantined
 * (`CryptoAccountView.quarantine`) joined with what the token list knows about the contract
 * (symbol, warnings). A holding whose token the list no longer calls quarantined is dropped, so a
 * verdict that landed a moment ago does not leave a stale row.
 */
export function quarantineRows(holdings: QuarantinedHolding[], tokens: TokenView[]): QuarantineRow[] {
  const rows: QuarantineRow[] = []
  for (const holding of holdings) {
    const token = tokens.find((entry) => entry.chain === holding.chain && entry.contract === holding.contract)
    if (token?.status !== 'quarantined') continue
    rows.push({
      id: `${holding.chain}|${holding.contract}`,
      chain: holding.chain,
      contract: holding.contract,
      symbol: token.symbol,
      name: token.name,
      quantity: holding.quantity,
      firstSeen: holding.first_seen,
      warnings: token.warnings,
    })
  }
  return rows.sort((left, right) => left.firstSeen.localeCompare(right.firstSeen))
}
