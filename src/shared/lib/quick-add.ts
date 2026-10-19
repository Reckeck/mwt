import { ref } from 'vue'

/**
 * Whether the «+ Операция» dialog is open. One flag for the whole app: the
 * shell draws the dialog, and any widget (the checklist, an empty state) may
 * open it without reaching up into the shell.
 */
export const quickAddOpen = ref(false)
