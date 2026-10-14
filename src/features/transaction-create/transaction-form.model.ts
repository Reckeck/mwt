/**
 * The operation form's model: pure functions over `TransactionFormState`,
 * split by concern under `lib/form/` (state, input building, linked
 * operations, validation, loading a stored operation, choices).
 */
export * from './lib/form'
export { toIsoTimestamp, toLocalInput } from '@/shared/lib/datetime'
