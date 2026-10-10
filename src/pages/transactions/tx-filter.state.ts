import { ref } from 'vue'

import { emptyTxFilter } from '@/shared/lib/tx-filter'

/**
 * The list and the graph tabs share one filter, and it survives switching
 * tabs: module state, alive as long as the app is.
 */
export const sharedTxFilter = ref<TxFilter>(emptyTxFilter())
