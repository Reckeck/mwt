import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { CRYPTO_OP_QUERY_KEYS, cryptoOpFilterFromQuery, cryptoOpFilterToQuery } from './crypto-op-filter'

/**
 * The filter and the open card live in the address (`?chain=&token=&…&op=`), so a link from another screen
 * lands on a narrowed list and «Back» returns to it. `accountIds` are the accounts the screen fixes.
 */
export function useCryptoOpRoute(accountIds: () => string[]) {
  const route = useRoute()
  const router = useRouter()

  const filter = computed(() => cryptoOpFilterFromQuery(route.query, accountIds()))
  function setFilter(next: CryptoOpFilter) {
    const rest = { ...route.query }
    for (const key of CRYPTO_OP_QUERY_KEYS) delete rest[key]
    void router.replace({ query: { ...rest, ...cryptoOpFilterToQuery(next, accountIds().length > 0) } })
  }

  const openId = computed(() => (typeof route.query.op === 'string' ? route.query.op : ''))
  function open(id: string) {
    void router.push({ query: { ...route.query, op: id } })
  }
  function close() {
    const { op: _op, ...rest } = route.query
    void router.replace({ query: rest })
  }

  return { filter, setFilter, openId, open, close }
}
