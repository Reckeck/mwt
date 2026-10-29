import { type MaybeRefOrGetter, computed, toValue } from 'vue'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

/**
 * Today's price of `asset` in `currency` — the `Now N` under the alert's
 * level (Auto-2 · 11). `null` while there is no cached price or no rate.
 */
export function useCurrentPrice(asset: MaybeRefOrGetter<AssetRef | null>, currency: MaybeRefOrGetter<string>) {
  const query = useQuery({
    queryKey: computed(() => {
      const picked = toValue(asset)
      return [
        'alert-current-price',
        picked?.symbol,
        picked?.coingecko_id,
        picked?.chain,
        picked?.contract,
        toValue(currency),
      ]
    }),
    queryFn: async () => {
      const picked = toValue(asset) as AssetRef
      const cached = await call<CachedPrice>('get_cached_price', {
        input: {
          symbol: picked.symbol,
          coingecko_id: picked.coingecko_id,
          chain: picked.chain,
          contract: picked.contract,
          at: null,
        },
      })
      // No price yet comes back as zero.
      if (!Number(cached.price_usd)) return null
      const code = toValue(currency).toLowerCase()
      if (code === 'usd') return Number(cached.price_usd)
      const input: ConvertAmountInput = { from: 'usd', to: code, amount: cached.price_usd, at: null }
      const converted = await call<ConvertedAmount>('convert_amount', { input })
      return converted.amount === null ? null : Number(converted.amount)
    },
    enabled: computed(() => Boolean(toValue(asset)) && Boolean(toValue(currency))),
    placeholderData: (previous) => previous,
    staleTime: 60_000,
  })
  return computed(() => query.data.value ?? null)
}
