export interface CryptoRefRowProps {
  /** The saved fiat record; `null` while the record is new (the pointer is written after the first save). */
  txId: string | null
  /** RFC3339 of the record, or empty for «now». */
  time: string
  amount?: string
  currency?: string
}
