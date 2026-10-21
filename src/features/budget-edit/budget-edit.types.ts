export interface BudgetDialogProps {
  /** A budget to edit; `null` creates a new one. */
  budget?: Budget | null
  /** A category the new limit watches (from the category card). */
  category?: string
}
