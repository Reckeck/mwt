<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { Button } from '@/shared/ui/button'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/shared/ui/table'
import { currencySymbol, formatAsOf, formatPct, formatQuantity } from '@/shared/lib/format'
import { useIsPhone } from '@/shared/lib/phone'
import { cn } from '@/shared/lib/utils'

/**
 * What a wallet holds, a row per asset. Plain (Wallets-1 · 6): asset,
 * quantity, price, value. Spot (Currency-1 · 4): also the 24 h change and the
 * share as a bar, under «Спот по валютам» with the time of the quotes. A
 * phone lists the same rows. A row opens the asset.
 */
const props = defineProps<{
  positions: StorePosition[]
  base: string
  decimals: number
  spot?: boolean
  pricesAsOf?: string | null
}>()
const emit = defineEmits<{ open: [position: StorePosition] }>()
const { t } = useI18n()
const phone = useIsPhone()

const total = computed(() => props.positions.reduce((sum, position) => sum + Math.max(0, Number(position.value_base)), 0))
const rows = computed(() =>
  [...props.positions]
    .sort((left, right) => Number(right.value_base) - Number(left.value_base))
    .map((position) => {
      const change = position.change_24h_pct === null ? null : Number(position.change_24h_pct)
      return {
        position,
        id: `${position.asset.symbol}-${position.asset.contract ?? ''}`,
        symbol: position.asset.symbol.toUpperCase(),
        quantity: formatQuantity(position.quantity, props.decimals),
        priced: Number(position.price_base) > 0,
        change: change === null ? '—' : formatPct(change, { signed: change !== 0, decimals: 1 }),
        tone: change === null || change === 0 ? 'text-muted-foreground' : change > 0 ? 'text-success' : 'text-destructive',
        share: total.value ? Math.max(0, Number(position.value_base)) / total.value : 0,
      }
    }),
)
const asOf = computed(() => (props.pricesAsOf ? formatAsOf(props.pricesAsOf) : ''))
const TH = 'h-9 px-3 text-xs font-medium text-muted-foreground'
</script>

<template>
  <section class="flex flex-col gap-3" data-tour="wallet-positions">
    <div v-if="props.spot && phone" class="flex items-baseline justify-between gap-2">
      <h3 class="heading-card">{{ t('wallet.spotByCurrency') }}</h3>
      <span v-if="asOf" class="text-xs text-muted-foreground">{{ asOf }}</span>
    </div>

    <!-- A phone: one line per asset, the value on the right. -->
    <div v-if="phone" class="rounded-xl border border-border bg-card px-3 shadow-sm">
      <Button
        v-for="row in rows"
        :key="row.id"
        variant="ghost"
        class="h-auto min-h-13 w-full justify-start gap-3 rounded-none border-b border-border px-0 py-1.5 text-left font-normal last:border-0 hover:bg-transparent"
        @click="emit('open', row.position)"
      >
        <span v-if="props.spot" class="flex size-9 shrink-0 items-center justify-center rounded-[10px] bg-muted text-2xs font-semibold">
          {{ row.symbol }}
        </span>
        <span class="flex min-w-0 flex-1 flex-col">
          <span class="truncate font-medium">{{ props.spot ? `${row.quantity} ${row.symbol}` : row.symbol }}</span>
          <span class="truncate text-xs text-muted-foreground">
            <template v-if="props.spot && row.priced">{{ t('wallet.assets.priceShort') }} <Money :amount="row.position.price_base" :currency="props.base" /></template>
            <template v-else-if="!props.spot">{{ row.quantity }}</template>
          </span>
        </span>
        <span class="flex flex-col items-end">
          <span class="font-medium tabular-nums"><Money :amount="row.position.value_base" :currency="props.base" :decimals="2" /></span>
          <span v-if="props.spot" :class="cn('text-xs', row.tone)">{{ row.change }}</span>
        </span>
      </Button>
    </div>

    <div v-else class="overflow-hidden rounded-xl border border-border bg-card shadow-sm">
      <div v-if="props.spot" class="flex items-baseline justify-between gap-2 px-4 pt-3 pb-1">
        <h3 class="text-base font-semibold">{{ t('wallet.spotByCurrency') }}</h3>
        <span v-if="asOf" class="text-xs text-muted-foreground">{{ t('wallet.ratesAt', { time: asOf }) }}</span>
      </div>
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead :class="TH">{{ props.spot ? t('wallet.assets.currency') : t('pos.asset') }}</TableHead>
            <TableHead :class="[TH, 'text-right']">{{ props.spot ? t('wallet.assets.howMuch') : t('pos.quantity') }}</TableHead>
            <TableHead :class="[TH, 'text-right']">{{ t('wallet.assets.price') }}</TableHead>
            <TableHead :class="[TH, 'text-right']">
              {{ props.spot ? t('wallet.assets.inBase', { sign: currencySymbol(props.base) }) : t('pos.value') }}
            </TableHead>
            <template v-if="props.spot">
              <TableHead :class="[TH, 'text-right']">{{ t('wallet.assets.change24h') }}</TableHead>
              <TableHead :class="TH">{{ t('wallet.assets.share') }}</TableHead>
            </template>
          </TableRow>
        </TableHeader>
        <TableBody>
          <TableRow v-for="row in rows" :key="row.id" class="cursor-pointer" @click="emit('open', row.position)">
            <TableCell class="px-3 py-2.5 font-medium">{{ row.symbol }}</TableCell>
            <TableCell class="px-3 py-2.5 text-right tabular-nums">{{ row.quantity }}</TableCell>
            <TableCell class="px-3 py-2.5 text-right text-muted-foreground tabular-nums">
              <Money v-if="row.priced" :amount="row.position.price_base" :currency="props.base" />
              <span v-else>—</span>
            </TableCell>
            <TableCell class="px-3 py-2.5 text-right font-semibold tabular-nums">
              <Money :amount="row.position.value_base" :currency="props.base" :decimals="2" />
            </TableCell>
            <template v-if="props.spot">
              <TableCell :class="cn('px-3 py-2.5 text-right tabular-nums', row.tone)">{{ row.change }}</TableCell>
              <TableCell class="w-[22%] px-3 py-2.5">
                <div class="h-2 overflow-hidden rounded bg-muted" role="img" :aria-label="formatPct(row.share * 100, { decimals: 0 })">
                  <div class="h-full rounded bg-primary" :style="{ width: `${row.share * 100}%` }" />
                </div>
              </TableCell>
            </template>
          </TableRow>
        </TableBody>
      </Table>
    </div>
  </section>
</template>
