/** What the category spent, for the hints of a limit opened from `Spending`. */
export interface BudgetSpending {
  /** The page's period, in `currency`. */
  spent: string
  /** The category's usual month, if known. */
  average: string | null
  currency: string
  /** The period as the hint names it: `in October`. */
  when: string
}

export interface BudgetDialogProps {
  /** A budget to edit; `null` creates a new one. */
  budget?: Budget | null
  /** A category the new limit watches (from the category card). */
  category?: string
  /** Its spending, when the dialog is opened from a page that shows it. */
  spending?: BudgetSpending | null
}

/** The fields the limit wizard checks. */
export type BudgetField = 'category' | 'store' | 'limit' | 'currency'
