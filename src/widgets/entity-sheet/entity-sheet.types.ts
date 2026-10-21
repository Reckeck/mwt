export interface EntitySheetProps {
  /** The card of the top entity; `null` while the first load runs. */
  card: EntityCard | null
  loading?: boolean
  error?: unknown
  /** More than one card is open: «Назад» shows the one under this. */
  canGoBack?: boolean
}

export interface EntityRelationsProps {
  groups: RelationGroup[]
}
