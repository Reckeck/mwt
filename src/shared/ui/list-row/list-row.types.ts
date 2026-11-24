import type { Component } from 'vue'

export type ListRowTone = 'ink' | 'positive' | 'negative'

export interface ListRowProps {
  title: string
  subtitle?: string
  /** Icon in the round mark at the start; without it there is no mark. */
  icon?: Component
  /** The figure at the end, already formatted (or use the `#value` slot for a `<Money>`). */
  value?: string
  /** Colour of the value: ink by default, positive green, negative red. */
  tone?: ListRowTone
}
