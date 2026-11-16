/** Protocol families by the name the operations list filters on; proper names, not translated. */
export const PROTOCOL_LABELS: Record<string, string> = {
  uniswap: 'Uniswap',
  aave: 'Aave',
  lido: 'Lido',
}

/** One protocol's own version label, e.g. `Uniswap v3`. */
export const VERSION_LABELS: Record<string, string> = {
  'uniswap-v3': 'Uniswap v3',
  'uniswap-v4': 'Uniswap v4',
  'aave-v3': 'Aave v3',
  lido: 'Lido',
}

export const protocolLabel = (protocol: string): string => VERSION_LABELS[protocol] ?? protocol

export const familyLabel = (family: string): string => PROTOCOL_LABELS[family] ?? family

/** The family a protocol belongs to: the key of the operations filter (`uniswap-v3` is `uniswap`). */
export const protocolFamily = (protocol: string): string => protocol.split('-')[0]

/** The query of the operations list that narrows it to what this position touched (`TrackingRef::Position`). */
export function operationsQuery(position: CryptoPositionView): Record<string, string> {
  const ref = 'position' in position.tracking ? position.tracking.position : undefined
  // No protocol key: the backend compares `CryptoTx::protocol` string for string
  // (`service/cryptotrack/operations.rs`), and the normalizer writes `None` there
  // (`cryptotrack/engine/normalize.rs`), so any value at all would empty the list.
  // The network and the account are what an event really carries; narrowing to the
  // position itself needs a `TrackingRef::Position` field in `CryptoOpFilter`.
  return {
    chain: ref?.chain ?? position.chain,
    account: position.account_id,
  }
}
