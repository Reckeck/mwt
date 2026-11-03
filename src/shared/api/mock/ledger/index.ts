/**
 * Browser-mock half of the ledger: transactions, contacts, categories and
 * rules for `pnpm dev`. Mirrors the Rust shapes closely enough to click
 * through the UI; the real rules (V1–V14) live only in the backend.
 */

export { mockCategories, mockContacts, mockRules, mockKindsFor } from './seed'
export { mockTransactionFrom } from './transaction-from'
export * from './contacts'
export * from './contact-summaries'
