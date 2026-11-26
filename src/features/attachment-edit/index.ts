/**
 * Files and links picked in a form before the operation exists.
 * - `PendingAttachments`: the list with its file and link buttons (`v-model` of `PendingAttachment[]`).
 * - `useAttachPending`: attaches them to the operation once it is saved.
 */
export { default as PendingAttachments } from './ui/PendingAttachments.vue'
export { useAttachPending } from './model/use-attach-pending'
export type { PendingAttachment } from './lib/pending-attachment'
