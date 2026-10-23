export interface GoalDeleteProps {
  /** The goal with what is put aside for it (`get_goal_progress`). */
  progress: GoalProgress
  /** Name of the wallet the money stays in. */
  wallet: string | null
  /** Name of the automation that fills the goal; it is switched off. */
  filler: string | null
  /** The button's text: «Удалить» in the card, «Удалить цель» in the edit form. */
  label?: string
}
