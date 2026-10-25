export interface EntitySheetProps {
  /** The card of the top entity; `null` while the first load runs. */
  card: EntityCard | null
  loading?: boolean
  error?: unknown
}

export interface EntityRelationsProps {
  groups: RelationGroup[]
}
