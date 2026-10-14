import type { Component } from 'vue'

/** The placeholder drawn while loading, shaped like what will replace it. */
export type SectionSkeleton = 'list' | 'chart' | 'figure' | 'table'

export interface SectionStateProps {
  /** First load in progress (TanStack `isPending`); a refetch keeps the content. */
  loading?: boolean
  /** The query's error (`query.error.value`); anything truthy shows the error block. */
  error?: unknown
  /** Loaded, and there is nothing to show. */
  empty?: boolean
  emptyTitle?: string
  emptyDescription?: string
  emptyIcon?: Component
  skeleton?: SectionSkeleton
  /** Rows of the list/table skeleton. */
  rows?: number
  /** Tighter empty and error blocks, for widgets and cards. */
  compact?: boolean
}
