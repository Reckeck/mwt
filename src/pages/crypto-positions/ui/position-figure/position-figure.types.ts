import type { MissingReason } from '@/entities/crypto-position'

export interface PositionFigureProps {
  /** A USD amount; `null`/`undefined` means the figure is not known and prints a dash, never zero. */
  amount?: string | number | null
  /** The engine's flag: the number is an approximation. It is shown as `≈` right next to the number. */
  approximate?: boolean
  /** Sign and colour by direction (a result), not for a balance. */
  colored?: boolean
  decimals?: number
  /** Why the figure is absent; shown as the dash's tooltip and read out by screen readers. */
  reason?: MissingReason
  /** The tokens without a price, for the `noPrice` reason. */
  tokens?: string
}
