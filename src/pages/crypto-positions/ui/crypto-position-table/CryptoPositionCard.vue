<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { PnlText } from '@/entities/currency'
import { Button } from '@/shared/ui/button'
import { ActionMenu } from '@/shared/ui/action-menu'
import type { UiAction } from '@/shared/ui/context-menu'
import { Card } from '@/shared/ui/card'

import { accrualFigure, feesFigure, protocolLabel, unpricedSymbols, valueFigure } from '@/entities/crypto-position'
import { PositionFigure } from '../position-figure'
import { PositionRate } from '../position-rate'
import PositionBadges from './PositionBadges.vue'

/** A position on a phone: a card with the same figures as the table row, two columns, the «⋯» in the corner. */
const props = defineProps<{ position: CryptoPositionView; actions: UiAction[] }>()
const emit = defineEmits<{ open: [] }>()
const { t } = useI18n()

const value = computed(() => valueFigure(props.position))
const accrual = computed(() => accrualFigure(props.position))
const fees = computed(() => feesFigure(props.position))
const tokens = computed(() => unpricedSymbols(props.position).join(', '))
</script>

<template>
  <Card size="sm" class="min-w-0">
    <div class="flex items-start gap-2">
      <Button variant="ghost" size="block" class="h-auto min-w-0 flex-1 flex-col items-start overflow-hidden" @click="emit('open')">
        <p class="w-full truncate text-left font-medium">{{ protocolLabel(props.position.protocol) }} · {{ props.position.name }}</p>
        <PositionBadges :position="props.position" class="mt-1" />
      </Button>
      <div class="flex shrink-0 flex-col items-end">
        <span v-if="props.position.net_usd !== undefined" class="inline-flex items-baseline gap-[0.25em] font-medium">
          <span v-if="props.position.approximate" class="text-muted-foreground">≈</span>
          <PnlText :amount="props.position.net_usd" currency="usd" :decimals="2" />
        </span>
        <PositionFigure v-else :amount="null" :reason="tokens ? 'noPrice' : 'notRead'" :tokens="tokens" />
        <span class="text-xs text-muted-foreground">{{ t('cryptoTrack.positions.net') }}</span>
      </div>
      <ActionMenu :actions="props.actions" :label="t('cryptoTrack.positions.actions.label')" class="-mr-1" />
    </div>
    <dl class="mt-3 grid grid-cols-2 gap-x-4 gap-y-2 text-sm">
      <div class="min-w-0">
        <dt class="truncate text-xs text-muted-foreground">{{ t('cryptoTrack.positions.putIn') }}</dt>
        <dd><PositionFigure :amount="props.position.put_in_usd" reason="noHistory" /></dd>
      </div>
      <div class="min-w-0">
        <dt class="truncate text-xs text-muted-foreground">{{ t('cryptoTrack.positions.col.value') }}</dt>
        <dd><PositionFigure :amount="value.amount" :approximate="props.position.approximate" :reason="value.reason" :tokens="tokens" /></dd>
      </div>
      <div class="min-w-0">
        <dt class="truncate text-xs text-muted-foreground">{{ t('cryptoTrack.positions.col.accrual') }}</dt>
        <dd><PositionFigure :amount="accrual.amount" :approximate="props.position.approximate" :reason="accrual.reason" :tokens="tokens" /></dd>
      </div>
      <div class="min-w-0" data-tour="defi-fees">
        <dt class="truncate text-xs text-muted-foreground">{{ t('cryptoTrack.positions.col.fees') }}</dt>
        <dd><PositionFigure :amount="fees.amount" :approximate="props.position.approximate" :reason="fees.reason" /></dd>
      </div>
      <div class="min-w-0">
        <dt class="truncate text-xs text-muted-foreground">APR</dt>
        <dd><PositionRate :apr="props.position.apr" :approximate="props.position.approximate" /></dd>
      </div>
    </dl>
  </Card>
</template>
