export interface GoalDialogProps {
  /** A goal to edit; `null` creates a new one. */
  goal?: Goal | null
  /** A new goal filled in advance: a goal made from a calculator scenario. */
  draft?: GoalInput | null
  /** A wallet the new goal's savings sit in («Сделать цель» from a wallet). */
  storeId?: string
}
