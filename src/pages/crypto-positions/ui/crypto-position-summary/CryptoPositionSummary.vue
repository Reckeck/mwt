<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { PnlText } from '@/entities/currency'
import { useIsPhone } from '@/shared/lib/phone'
import { Card } from '@/shared/ui/card'
import { StatCard } from '@/shared/ui/stat-card'

import { netPercent, sumOf, unpricedSymbols } from '@/entities/crypto-position'
import { PositionFigure } from '../position-figure'
import type { CryptoPositionSummaryProps } from './crypto-position-summary.types'

/**
 * Put in, worth now, net result over the whole list. Each total counts only the positions that know the figure;
 * the ones that do not are named in the hint, so a total is never a quiet underestimate.
 *
 * On a computer they are three tiles; on a phone they are three rows of one card (the canvas board «14 DeFi»),
 * because three stacked tiles push the list itself below the fold and three narrow tiles cut the hints off.
 */
const props = defineProps<CryptoPositionSummaryProps>()
const { t } = useI18n()
const phone = useIsPhone()

const putIn = computed(() => sumOf(props.positions, (position) => position.put_in_usd))
const value = computed(() => sumOf(props.positions, (position) => position.value_usd))
const net = computed(() => sumOf(props.positions, (position) => position.net_usd))
const percent = computed(() => netPercent(props.positions))
const approximate = computed(() => props.positions.some((position) => position.approximate))

const hint = (missing: number) => (missing ? t('cryptoTrack.positions.summary.missing', { count: missing }) : undefined)

/** The tokens behind a dash, so the reason names them instead of leaving a gap in the sentence. */
const tokens = computed(() => [...new Set(props.positions.flatMap(unpricedSymbols))].join(', '))
const reason = computed(() => (tokens.value ? 'noPrice' : 'notRead'))
</script>

<template>
  <Card v-if="phone" size="sm">
    <dl class="grid gap-2">
      <div class="flex items-baseline justify-between gap-3">
        <dt class="min-w-0 text-sm text-muted-foreground" :title="t('cryptoTrack.positions.hint.putIn')">
          {{ t('cryptoTrack.positions.putIn') }}
          <span v-if="hint(putIn.missing)" class="block text-xs">{{ hint(putIn.missing) }}</span>
        </dt>
        <dd class="shrink-0 font-medium tabular-nums"><PositionFigure :amount="putIn.sum" reason="noHistory" /></dd>
      </div>
      <div class="flex items-baseline justify-between gap-3">
        <dt class="min-w-0 text-sm text-muted-foreground" :title="t('cryptoTrack.positions.hint.value')">
          {{ t('cryptoTrack.positions.value') }}
          <span v-if="hint(value.missing)" class="block text-xs">{{ hint(value.missing) }}</span>
        </dt>
        <dd class="shrink-0 font-medium tabular-nums">
          <PositionFigure :amount="value.sum" :approximate="approximate" :reason="reason" :tokens="tokens" />
        </dd>
      </div>
      <div class="flex items-baseline justify-between gap-3" data-tour="defi-position-net">
        <dt class="min-w-0 text-sm text-muted-foreground">
          {{ t('cryptoTrack.positions.net') }}
          <span v-if="hint(net.missing)" class="block text-xs">{{ hint(net.missing) }}</span>
        </dt>
        <dd class="shrink-0 font-medium tabular-nums">
          <span v-if="net.sum === null" class="text-muted-foreground" :title="t(`cryptoTrack.positions.reason.${reason}`, { tokens })">—</span>
          <span v-else class="inline-flex items-baseline gap-[0.25em]">
            <span v-if="approximate" class="text-muted-foreground" :title="t('cryptoTrack.positions.approximateMark')">≈</span>
            <PnlText :amount="net.sum" currency="usd" :decimals="2" :pct="percent" />
          </span>
        </dd>
      </div>
    </dl>
  </Card>
  <div v-else class="grid grid-cols-1 gap-3 sm:grid-cols-3">
    <StatCard :label="t('cryptoTrack.positions.putIn')" :hint="hint(putIn.missing)" :title="t('cryptoTrack.positions.hint.putIn')">
      <PositionFigure :amount="putIn.sum" reason="noHistory" />
    </StatCard>
    <StatCard :label="t('cryptoTrack.positions.value')" :hint="hint(value.missing)" :title="t('cryptoTrack.positions.hint.value')">
      <PositionFigure :amount="value.sum" :approximate="approximate" :reason="reason" :tokens="tokens" />
    </StatCard>
    <StatCard :label="t('cryptoTrack.positions.net')" :hint="hint(net.missing)" data-tour="defi-position-net">
      <span v-if="net.sum === null" class="text-muted-foreground" :title="t(`cryptoTrack.positions.reason.${reason}`, { tokens })">—</span>
      <span v-else class="inline-flex items-baseline gap-[0.25em]">
        <span v-if="approximate" class="text-muted-foreground" :title="t('cryptoTrack.positions.approximateMark')">≈</span>
        <PnlText :amount="net.sum" currency="usd" :decimals="2" :pct="percent" />
      </span>
    </StatCard>
  </div>
</template>
