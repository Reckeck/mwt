import type { Component } from 'vue'

export interface EmptyStateProps {
  /** What is missing, in the user's words ("No goals yet"); a generic line when omitted. */
  title?: string
  /** What will be here and how to start; the action button goes in the default slot. */
  description?: string
  /** A lucide icon shown above the title. */
  icon?: Component
  /** Tighter padding for a block inside a card or a widget. */
  compact?: boolean
}
