export interface AutomationDialogProps {
  /** An automation to edit; `null` creates a new one. */
  automation?: Automation | null
  /** Wallet a new automation starts on. */
  storeId?: string
}

export interface AutomationRunActionsProps {
  run: AutomationRun
}
