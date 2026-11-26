/**
 * Export surface of the quick operation form.
 * - `QuickForm`: kind switch, hero amount with keypad, wallet, category, date (shows the errors it is given).
 * - `AccountRefPicker`: the wallet list of an operation.
 * - `lib/quick-form`: the form state, its rules (`quickErrors`, rule 13) and the backend input.
 * - `useQuickAsset`, `useQuickSave`: the asset a wallet is written in; the write.
 */
export { default as QuickForm } from './ui/quick-form/QuickForm.vue'
export * from './ui/account-ref-picker'
export * from './lib/quick-form'
export { quickWalletNodes } from './lib/quick-wallets'
export { useQuickAsset } from './model/use-quick-asset'
export { useQuickSave } from './model/use-quick-save'
