<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import Card from '@/components/ui/card.vue'
import { CHAIN_LABELS } from '@/lib/chains'
import { formatMoney } from '@/lib/format'
import { cn } from '@/lib/utils'

const props = defineProps<{ positions: StorePosition[]; baseCurrency: string; decimals: number }>()

const { t } = useI18n()

const b = computed(() => props.baseCurrency.toUpperCase())

const money = (v: string) => formatMoney(v, { currency: b.value, decimals: 2 })
const qty = (v: string) => formatMoney(v, { decimals: props.decimals })

function toneOf(v: string): string {
  const n = Number(v)
  if (n > 0) return 'text-emerald-600 dark:text-emerald-400'
  if (n < 0) return 'text-destructive'
  return ''
}
</script>

<template>
  <Card>
    <template #header>
      <p class="font-medium">{{ t('pos.title') }}</p>
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
          </tr>
        </thead>
        <tbody class="divide-y divide-border">
          <tr v-for="p in positions" :key="`${p.asset.symbol}-${p.asset.contract ?? ''}`">
            <td class="py-2 pr-2">
              <div class="flex items-center gap-2">
                <span class="font-medium">{{ p.asset.symbol.toUpperCase() }}</span>
                <span class="truncate text-xs text-muted-foreground">
                  {{ p.asset.chain && p.asset.chain !== 'fiat' ? CHAIN_LABELS[p.asset.chain] : p.asset.name }}
                </span>
              </div>
            </td>
            <td class="py-2 text-right tabular-nums">{{ qty(p.quantity) }}</td>
            <td class="py-2 text-right tabular-nums text-muted-foreground">
              {{ p.avg_cost ? money(p.avg_cost) : '—' }}
            </td>
            <td class="py-2 text-right tabular-nums text-muted-foreground">
              {{ Number(p.price_usd) > 0 ? money(p.price_usd) : '—' }}
            </td>
            <td class="py-2 text-right font-medium tabular-nums">{{ money(p.value_base) }}</td>
            <td class="py-2 text-right tabular-nums" :class="cn(toneOf(p.unrealized_base))">
              {{ p.avg_cost ? money(p.unrealized_base) : '—' }}
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <p v-else class="text-sm text-muted-foreground">{{ t('pos.empty') }}</p>
  </Card>
</template>
