export interface EntityLinkProps {
  target: EntityRef
  /** The name to show; empty falls back to the kind («Операция»). */
  label?: string
  /** Archived or switched off: the link is dimmed, still opens. */
  archived?: boolean
  /** Hide the kind icon where the column already says what it is. */
  plain?: boolean
}
