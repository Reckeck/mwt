export interface GoalDetailsSheetProps {
  goalId: string | null
  /** The goal's links from its entity card: its wallets and what fills them. */
  groups?: RelationGroup[]
}
