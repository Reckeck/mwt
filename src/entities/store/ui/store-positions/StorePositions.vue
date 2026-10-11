<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Card } from '@/shared/ui/card'
import { CHAIN_LABELS } from '@/shared/lib/chains'
import { formatMoney } from '@/shared/lib/format'
import { cn } from '@/shared/lib/utils'

import type { StorePositionsProps } from './store-positions.types'

const props = withDefaults(defineProps<StorePositionsProps>(), { title: '' })

/** Per-row actions (revalue a property…) come from the page. */
defineSlots<{ actions?(props: { position: StorePosition }): unknown }>()

const { t } = useI18n()

const base = computed(() => props.baseCurrency.toUpperCase())

const money = (value: string) => formatMoney(value, { currency: base.value, decimals: 2 })
const qty = (value: string) => formatMoney(value, { decimals: props.decimals })

function toneOf(value: string): string {
  const amount = Number(value)
  if (amount > 0) return 'text-emerald-600 dark:text-emerald-400'
  if (amount < 0) return 'text-destructive'
  return ''
}
</script>

<template>
  <Card>
    <template #header>
      <p class="font-medium">{{ props.title || t('pos.title') }}</p>
    </template>

    <div v-if="positions.length" class="overflow-x-auto">
      <table class="w-full min-w-[34rem] text-sm">
        <thead>
          <tr class="border-b border-border text-left text-xs uppercase tracking-wide text-muted-foreground">
            <th class="pb-2 font-medium">{{ t('pos.asset') }}</th>
            <th class="pb-2 text-right font-medium">{{ t('pos.quantity') }}</th>
            <th class="pb-2 text-right font-medium">{{ t('pos.avgCost') }}</th>
            <th class="pb-2 text-right font-medium">{{ t('pos.price') }}</th>
            <th class="pb-2 text-right font-medium">{{ t('pos.value') }}</th>
            <th class="pb-2 text-right font-medium">{{ t('pos.pnl') }}</th>
            <th v-if="$slots.actions" class="pb-2" />
          </tr>
        </thead>
        <tbody class="divide-y divide-border">
          <tr v-for="position in positions" :key="`${position.asset.symbol}-${position.asset.contract ?? ''}`">
            <td class="py-2 pr-2">
              <div class="flex items-center gap-2">
                <span class="font-medium">{{ position.asset.symbol.toUpperCase() }}</span>
                <span class="truncate text-xs text-muted-foreground">
                  {{ position.asset.chain && position.asset.chain !== 'fiat' ? CHAIN_LABELS[position.asset.chain] : position.asset.name }}
                </span>
              </div>
            </td>
            <td class="py-2 text-right tabular-nums">{{ qty(position.quantity) }}</td>
            <td class="py-2 text-right tabular-nums text-muted-foreground">
              {{ position.avg_cost ? money(position.avg_cost) : '—' }}
            </td>
            <td class="py-2 text-right tabular-nums text-muted-foreground">
              {{ Number(position.price_base) > 0 ? money(position.price_base) : '—' }}
            </td>
            <td class="py-2 text-right font-medium tabular-nums">{{ money(position.value_base) }}</td>
            <td class="py-2 text-right tabular-nums" :class="cn(toneOf(position.unrealized_base))">
              {{ position.avg_cost ? money(position.unrealized_base) : '—' }}
            </td>
            <td v-if="$slots.actions" class="py-2 pl-2 text-right">
              <slot name="actions" :position="position" />
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <p v-else class="text-sm text-muted-foreground">{{ t('pos.empty') }}</p>
  </Card>
</template>
