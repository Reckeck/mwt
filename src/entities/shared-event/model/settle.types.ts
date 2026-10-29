/** What a toast needs to say about a settlement besides the backend's answer. */
export type SettleKind = 'received' | 'gave' | 'paid' | 'offset'

export interface SettleChange {
  input: SettleSharedInput
  kind: SettleKind
  /** Who it was with: the person, or the organizer when I pay my share. */
  name: string
  currency: string
  /** Fraction digits of the sums in the toast (`sumDecimals`). */
  decimals: number
  /** The wallet's name, when money of mine moved. */
  walletName?: string
}

export interface RemainderChange {
  kind: 'write_off' | 'reassign'
  input: SharedRemainderInput
  name: string
  /** What is written off or moved: the request itself says «the whole rest» with no sum. */
  amount: string
  currency: string
  decimals: number
}

export interface DueChange {
  input: SetSharedDueInput
  /** The term before the change, for «Отменить». */
  previous: string | null
}
