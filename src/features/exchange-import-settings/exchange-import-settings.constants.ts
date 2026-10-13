/** Bybit's accounts, in the order the settings list them. */
export const IMPORT_ACCOUNTS = ['FUND', 'UNIFIED', 'EARN'] as const

/** What an import can bring in, each with its own switch and category. */
export const IMPORT_KINDS = ['trades', 'transfers', 'earn'] as const
