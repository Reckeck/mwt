export interface GoalDialogProps {
  /** A goal to edit; `null` creates a new one. */
  goal?: Goal | null
  /** A wallet the new goal's savings sit in («Сделать цель» from a wallet). */
  storeId?: string
}
