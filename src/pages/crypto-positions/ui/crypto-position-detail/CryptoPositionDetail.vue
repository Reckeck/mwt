<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { CircleCheck, Minus, Waves } from 'lucide-vue-next'

import { PnlText } from '@/entities/currency'
import { formatDate } from '@/shared/lib/format'
import { Alert, AlertDescription, AlertTitle } from '@/shared/ui/alert'
import { StatCard } from '@/shared/ui/stat-card'

import { protocolLabel, unpricedSymbols, valueFigure, withdrawnUsd } from '@/entities/crypto-position'
import PositionBadges from '../crypto-position-table/PositionBadges.vue'
import { PositionFigure } from '../position-figure'
import { PositionRate } from '../position-rate'
import PositionBreakdown from './PositionBreakdown.vue'
import PositionSideCard from './PositionSideCard.vue'

/**
 * One position whole: the header with the net result, three tiles, how the net is made up, and the block of its
 * kind. A strip on top says what the user must know to read the numbers: approximate, a token without a price,
 * a closed position frozen at its closing.
 */
const props = defineProps<{ position: CryptoPositionView }>()
const { t } = useI18n()

const closed = computed(() => props.position.status === 'closed')
const unpriced = computed(() => unpricedSymbols(props.position).join(', '))
const value = computed(() => valueFigure(props.position))
const withdrawn = computed(() => withdrawnUsd(props.position))
const subtitle = computed(() =>
  [
    props.position.opened_at ? t('cryptoTrack.positions.opened', { date: formatDate(props.position.opened_at) }) : null,
    props.position.closed_at ? t('cryptoTrack.positions.closedOn', { date: formatDate(props.position.closed_at) }) : null,
  ]
    .filter(Boolean)
    .join(' · '),
)
</script>

<template>
  <div class="grid gap-4">
    <Alert v-if="unpriced">
      <Minus />
      <AlertTitle>{{ t('cryptoTrack.positions.banner.noPriceTitle', { tokens: unpriced }) }}</AlertTitle>
      <AlertDescription>{{ t('cryptoTrack.positions.banner.noPrice') }}</AlertDescription>
    </Alert>
    <Alert v-if="props.position.approximate">
      <Waves />
      <AlertTitle>{{ t('cryptoTrack.positions.banner.approximateTitle') }}</AlertTitle>
      <AlertDescription>{{ t('cryptoTrack.positions.banner.approximate') }}</AlertDescription>
    </Alert>
    <Alert v-if="closed">
      <CircleCheck />
      <AlertTitle>{{ t('cryptoTrack.positions.banner.closedTitle') }}</AlertTitle>
      <AlertDescription>{{ t('cryptoTrack.positions.banner.closed') }}</AlertDescription>
    </Alert>

    <header class="flex flex-wrap items-start justify-between gap-3">
      <div class="min-w-0">
        <h2 class="truncate font-semibold">{{ protocolLabel(props.position.protocol) }} · {{ props.position.name }}</h2>
        <PositionBadges :position="props.position" class="mt-1" />
        <p v-if="subtitle" class="mt-1 text-xs text-muted-foreground">{{ subtitle }}</p>
      </div>
      <div class="ml-auto text-right">
        <p class="figure">
          <span v-if="props.position.net_usd === undefined" class="text-muted-foreground">—</span>
          <span v-else class="inline-flex items-baseline gap-[0.25em]">
            <span v-if="props.position.approximate" class="text-muted-foreground">≈</span>
            <PnlText :amount="props.position.net_usd" currency="usd" :decimals="2" />
          </span>
        </p>
        <p class="text-xs text-muted-foreground">
          {{ props.position.net_usd === undefined ? t('cryptoTrack.positions.netUnknown') : closed ? t('cryptoTrack.positions.netClosed') : t('cryptoTrack.positions.net') }}
        </p>
      </div>
    </header>

    <div class="grid gap-3 sm:grid-cols-3">
      <StatCard :label="t('cryptoTrack.positions.putIn')">
        <PositionFigure :amount="props.position.put_in_usd" reason="noHistory" />
      </StatCard>
      <StatCard v-if="closed" :label="t('cryptoTrack.positions.withdrawn')">
        <PositionFigure :amount="withdrawn" reason="noHistory" />
      </StatCard>
      <StatCard v-else :label="t('cryptoTrack.positions.value')">
        <PositionFigure :amount="value.amount" :approximate="props.position.approximate" :reason="value.reason" :tokens="unpriced" />
      </StatCard>
      <StatCard :label="closed ? t('cryptoTrack.positions.aprClosed') : 'APR'">
        <PositionRate :apr="props.position.apr" :approximate="props.position.approximate" />
      </StatCard>
    </div>

    <PositionBreakdown :position="props.position" />
    <PositionSideCard :position="props.position" />
  </div>
</template>
