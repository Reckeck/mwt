export interface SharedPurchaseDialogProps {
  event: SharedEventView
  /** The planned item that was bought; none records a purchase outside the plan (a new item). */
  item?: EventItem | null
  /** The purchase to change; none records a new one. */
  purchase?: Purchase | null
}

export interface AttachPaymentDialogProps {
  event: SharedEventView
}
