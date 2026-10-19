<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQueries } from '@tanstack/vue-query'

import { Money, PnlText } from '@/entities/currency'
import { call } from '@/shared/api/ipc'
import { assetKey } from '@/shared/lib/asset-ref'
import { txPrincipalLegs } from '@/shared/lib/transactions'

/** What a coin cost then against what it costs now, per principal leg that recorded a price. */
const props = defineProps<{ tx: Transaction }>()
const { t } = useI18n()

/** One row per coin: a currency's rate is not a price worth comparing. */
const legs = computed(() => {
  const seen = new Set<string>()
  return txPrincipalLegs(props.tx).filter((leg) => {
    const key = assetKey(leg.asset)
    if (!leg.price_usd || !Number(leg.price_usd) || leg.asset.chain === 'fiat' || seen.has(key)) return false
    seen.add(key)
    return true
  })
})

const nows = useQueries({
  queries: computed(() =>
    legs.value.map((leg) => ({
      queryKey: ['cached-price', assetKey(leg.asset), 'now'],
      queryFn: () =>
        call<CachedPrice>('get_cached_price', {
          input: {
            symbol: leg.asset.symbol,
            coingecko_id: leg.asset.coingecko_id,
            chain: leg.asset.chain,
            contract: leg.asset.contract,
            at: null,
          },
        }),
    })),
  ),
})

const rows = computed(() =>
  legs.value.map((leg, index) => {
    const now = nows.value[index]?.data
    const then = Number(leg.price_usd)
    const current = now?.ts ? Number(now.price_usd) : null
    const quantity = Math.abs(Number(leg.amount))
    return {
      key: assetKey(leg.asset),
      symbol: leg.asset.symbol.toUpperCase(),
      paid: leg.price_usd as string,
      now: current === null ? null : String(current),
      change: current === null ? null : String((current - then) * quantity),
      pct: current === null || !then ? null : ((current - then) / then) * 100,
    }
  }),
)
</script>

<template>
  <section v-if="rows.length" class="grid gap-1.5 text-sm">
    <h3 class="text-xs font-medium text-muted-foreground">{{ t('txDetails.prices') }}</h3>
    <dl v-for="row in rows" :key="row.key" class="grid grid-cols-[auto_1fr] gap-x-3 gap-y-0.5">
      <dt class="font-medium">{{ row.symbol }}</dt>
      <dd class="flex flex-wrap justify-end gap-x-3 text-right">
        <span>{{ t('txDetails.priceThen') }} <Money :amount="row.paid" currency="usd" /></span>
        <span v-if="row.now">{{ t('txDetails.priceNow') }} <Money :amount="row.now" currency="usd" /></span>
        <span v-else class="text-muted-foreground">{{ t('txDetails.priceNowUnknown') }}</span>
      </dd>
      <template v-if="row.change !== null">
        <dt class="text-xs text-muted-foreground">{{ t('txDetails.priceChange') }}</dt>
        <dd class="text-right text-xs"><PnlText :amount="row.change" currency="usd" :decimals="2" :pct="row.pct" /></dd>
      </template>
    </dl>
  </section>
</template>
