import type { Component } from 'vue'

export type ListRowTone = 'ink' | 'positive' | 'negative'

/** `default` is 56 px; `withControl` is 64 px, for a row with a switch beside it (keeps the 40 px hit area clear of the floating button). */
export type ListRowSize = 'default' | 'withControl'

export interface ListRowProps {
  title: string
  subtitle?: string
  /** Icon in the round mark at the start; without it there is no mark. */
  icon?: Component
  /** The figure at the end, already formatted (or use the `#value` slot for a `<Money>`). */
  value?: string
  /** Colour of the value: ink by default, positive green, negative red. */
  tone?: ListRowTone
  size?: ListRowSize
}
