import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { useIsPhone } from '@/shared/lib/phone'

/**
 * The picked contact lives in the address (`?contact=<id>`): the table writes it, the card reads it.
 * A desktop swaps the pick in place; a phone steps into the card, so «Back» closes it.
 */
export function useContactSelection() {
  const route = useRoute()
  const router = useRouter()
  const phone = useIsPhone()

  const selectedId = computed(() => (typeof route.query.contact === 'string' ? route.query.contact : ''))

  function select(id: string) {
    const to = { query: { ...route.query, contact: id } }
    void (phone.value && !selectedId.value ? router.push(to) : router.replace(to))
  }

  function clear() {
    if (phone.value && window.history.state?.back) {
      router.back()
      return
    }
    void router.replace({ query: { ...route.query, contact: undefined } })
  }

  return { selectedId, select, clear }
}
