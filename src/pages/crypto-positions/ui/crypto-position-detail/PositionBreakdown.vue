<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { PnlText } from '@/entities/currency'
import { formatPct } from '@/shared/lib/format'
import { Card } from '@/shared/ui/card'

import { positionBreakdown, unpricedSymbols } from '@/entities/crypto-position'
import { PositionFigure } from '../position-figure'

/**
 * How the net result is made up. The lines add up to the engine's net by its own identity
 * (`net = value change + fees collected + fees uncollected - gas`); impermanent loss is a note under the sum,
 * never a line of it, because it already sits inside the value change.
 */
const props = defineProps<{ position: CryptoPositionView }>()
const { t } = useI18n()

const lines = computed(() => positionBreakdown(props.position))
const tokens = computed(() => unpricedSymbols(props.position).join(', '))
const ratio = computed(() =>
  props.position.il_ratio === undefined ? '—' : formatPct(Number(props.position.il_ratio) * 100, { signed: true }),
)
const accrues = computed(() => props.position.kind !== 'pool')
</script>

<template>
  <Card data-tour="defi-position-breakdown">
    <template #header>
      <h3 class="heading-card">{{ t('cryptoTrack.positions.breakdown.title') }}</h3>
    </template>
    <dl class="grid gap-2 text-sm">
      <div v-for="line in lines" :key="line.id" class="flex items-baseline justify-between gap-3">
        <dt class="min-w-0 text-muted-foreground">{{ t(`cryptoTrack.positions.breakdown.${line.id}`) }}</dt>
        <dd class="shrink-0">
          <PositionFigure :amount="line.amount" :approximate="line.approximate" :reason="line.reason" :tokens="tokens" colored />
        </dd>
      </div>
      <div class="mt-1 flex items-baseline justify-between gap-3 border-t border-border pt-3 font-semibold">
        <dt>{{ t('cryptoTrack.positions.net') }}</dt>
        <dd>
          <span
            v-if="props.position.net_usd === undefined"
            class="text-muted-foreground"
            :title="t(`cryptoTrack.positions.reason.${tokens ? 'noPrice' : 'notRead'}`, { tokens })"
            >—</span
          >
          <span v-else class="inline-flex items-baseline gap-[0.25em]">
            <span v-if="props.position.approximate" class="text-muted-foreground">≈</span>
            <PnlText :amount="props.position.net_usd" currency="usd" :decimals="2" />
          </span>
        </dd>
      </div>
    </dl>
    <p class="mt-3 text-xs text-muted-foreground">
      <template v-if="accrues">{{ t('cryptoTrack.positions.breakdown.noteAccrues') }}</template>
      <template v-else-if="props.position.il_usd === undefined">{{ t('cryptoTrack.positions.breakdown.noteNoIl') }}</template>
      <i18n-t v-else keypath="cryptoTrack.positions.breakdown.noteIl" tag="span">
        <template #il>
          <PositionFigure :amount="props.position.il_usd" :approximate="props.position.approximate" colored />
        </template>
        <template #ratio>{{ ratio }}</template>
      </i18n-t>
    </p>
  </Card>
</template>
