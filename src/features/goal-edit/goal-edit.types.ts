export interface GoalDialogProps {
  /** A goal to edit; `null` creates a new one. */
  goal?: Goal | null
  /** A new goal filled in advance: a goal made from a calculator scenario. */
  draft?: GoalInput | null
  /** A wallet the new goal's savings sit in («Сделать цель» from a wallet). */
  storeId?: string
  /** What is left of a month, offered to go straight into the new goal («Куда ушли»). */
  remainder?: GoalRemainder | null
}

export interface GoalRemainder {
  amount: string
  currency: string
  /** `YYYY-MM`. */
  month: string
}

export type GoalField =
  | 'name'
  | 'target'
  | 'currency'
  | 'startsAt'
  | 'deadline'
  | 'commitment'
  | 'expectedReturn'
  | 'stores'
