export interface UtilityDialogProps {
  /** An item to edit; `null` creates a new one. */
  item?: UtilityItem | null
  /** The property a new bill belongs to (opened from that property). */
  propertyId?: string
}

export interface MeterFieldsProps {
  /** The bill's currency, for the preview. */
  currency: string
  /** `preview_utility_formula` of 100 units per zone, while typing. */
  preview?: UtilityFormulaPreview | null
  unitError?: string
  tariffError?: string
  zonesError?: string
  formulaError?: string
}
