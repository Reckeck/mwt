export interface UtilityDialogProps {
  /** An item to edit; `null` creates a new one. */
  item?: UtilityItem | null
  /** The property a new bill belongs to (opened from that property). */
  propertyId?: string
}
