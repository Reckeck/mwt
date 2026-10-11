export interface AutomationDialogProps {
  /** An automation to edit; `null` creates a new one. */
  automation?: Automation | null
  /** Wallet a new automation starts on. */
  storeId?: string
  /** A ready-made kind from the wallet: fills the name, operation kind and category. */
  preset?: PresetInfo | null
}

export interface AutomationRunActionsProps {
  run: AutomationRun
}
