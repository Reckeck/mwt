export interface AutomationListProps {
  /** Show only the automations touching this wallet (and its sub-accounts). */
  storeId?: string
  /** Ready-made automations the wallet offers, from the wallet catalog. */
  presets?: PresetInfo[]
  /** An overview: no creating here, that happens in the wallet. */
  readonly?: boolean
}
