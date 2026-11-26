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
  /** The automation's name, for the `How much arrived` dialog. */
  name?: string
}

/** The operation as typed in step 2: handed back to the form when the wallet changes. */
export interface WizardDraft {
  kind: TxKind
  categories: string[]
  amount?: string
  comment?: string
  to?: string
}

/** What the wizard reads from the operation form the page puts into step 2. */
export interface WizardOperationForm {
  valid: boolean
  showErrors: () => void
  input: TransactionInput
  draft: WizardDraft
}

/** What `#operation` gets to build the operation form of step 2. */
export interface WizardOperationSlot {
  storeId: string
  nodes: StoreTreeNode[]
  initial: WizardDraft | null
  /** Changes when the form must start over: key the form by it. */
  formKey: number
  /** The form's ref: step 3 reads what was typed from it. */
  bind: (element: unknown) => void
  submit: () => void
  changeStore: (id: string) => void
}

/** What the wizard hands on to another form: a reminder, a price alert, a scheme, a ready-made start. */
export type WizardHandoff =
  | { kind: 'reminder' }
  | { kind: 'alert' }
  | { kind: 'scheme' }
  | { kind: 'starter'; starter: 'save_share' | 'low_balance' }
