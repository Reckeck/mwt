import { computed } from 'vue'
import { useQueryClient } from '@tanstack/vue-query'

import { useSettingsStore } from '@/shared/store/settings'

/**
 * The user's favourite currencies, fiat and crypto alike: lowercase, in the
 * user's order (it ranks the conversions under an amount). Every list of
 * currencies or assets reads them here, so «is it a favourite» means the same
 * everywhere. Writes go to the app settings; the core uppercases and drops
 * codes it cannot use.
 */
export function useFavoriteCurrencies() {
  const settings = useSettingsStore()
  const queryClient = useQueryClient()

  const codes = computed(() => settings.favoriteCurrencies.map((code) => code.toLowerCase()))
  /** The base currency first, then the favourites: what lists put on top. */
  const withBase = computed(() => [...new Set([settings.baseCurrency, ...codes.value])])

  const isFavorite = (code: string) => codes.value.includes(code.trim().toLowerCase())

  async function save(next: string[]) {
    await settings.updateApp({ favorite_currencies: next.map((code) => code.toUpperCase()) })
    // The rates table has a row per crypto favourite.
    void queryClient.invalidateQueries({ queryKey: ['fx-table'] })
  }

  const add = (code: string) =>
    isFavorite(code) ? Promise.resolve() : save([...codes.value, code.trim().toLowerCase()])
  const remove = (code: string) => save(codes.value.filter((entry) => entry !== code.trim().toLowerCase()))
  const toggle = (code: string) => (isFavorite(code) ? remove(code) : add(code))

  /** Move a favourite from one place in the order to another. */
  function move(from: number, to: number) {
    const next = [...codes.value]
    const [moved] = next.splice(from, 1)
    if (moved === undefined) return Promise.resolve()
    next.splice(Math.max(0, Math.min(to, next.length)), 0, moved)
    return save(next)
  }

  return { codes, withBase, isFavorite, add, remove, toggle, move, save }
}
