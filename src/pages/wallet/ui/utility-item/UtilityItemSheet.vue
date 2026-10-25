<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { ChevronLeft, Pencil } from 'lucide-vue-next'

import { MoneyPhrase } from '@/entities/currency'
import { UtilityDialog } from '@/features/utility-edit'
import { Button } from '@/shared/ui/button'
import { SectionState } from '@/shared/ui/section-state'
import { Sheet, SheetContent, SheetDescription, SheetTitle } from '@/shared/ui/sheet'
import { call } from '@/shared/api/ipc'
import { formatMonthKey } from '@/shared/lib/format'
import type { SumPart } from '@/shared/lib/money-parts'
import { PLANNING_KEYS } from '@/shared/lib/planning'
import { cn } from '@/shared/lib/utils'

import { prettyFormula } from '@/shared/lib/utility-formula'

import { historyRows, itemSubtitle } from '../../lib/utility-item'
import UtilityItemHistory from './UtilityItemHistory.vue'
import UtilityReadingCard from './UtilityReadingCard.vue'

/**
 * One item of the bill (Wallets-2 · 11–12): the tariff, the six-month average
 * and the forecast; this month's reading with what it comes to; the history.
 * A formula item shows its formula on top. A side panel on a desktop, a
 * screen of its own on a phone.
 */
const props = defineProps<{ line: UtilityBillLine | null; period: string; phone: boolean; disabled: boolean }>()
const open = defineModel<boolean>('open', { required: true })
const { t } = useI18n()
const queryClient = useQueryClient()

const item = computed(() => props.line?.item ?? null)
const trend = useQuery({
  // no placeholder: another item's history must not show under this one
  queryKey: computed(() => ['utility-trend', item.value?.id]),
  queryFn: () => call<UtilityTrend>('get_meter_trend', { itemId: item.value?.id }),
  enabled: computed(() => open.value && Boolean(item.value)),
})
const meter = computed(() => (item.value && 'metered' in item.value.kind ? item.value.kind.metered : null))
const money = (value: string, decimals = 2): SumPart => ({ amount: value, currency: item.value?.currency ?? 'rub', decimals })
const month = computed(() => formatMonthKey(props.period, { month: 'long' }))
const stats = computed(() => {
  const data = trend.data.value
  const list: { label: string; value: SumPart[] }[] = []
  if (meter.value && !meter.value.formula)
    list.push({ label: t('utilityItem.tariff'), value: [money(meter.value.tariff), { text: ` / ${meter.value.unit}` }] })
  list.push({ label: t('utilityItem.average'), value: data?.average ? [money(data.average)] : [{ text: '—' }] })
  list.push({
    label: t('utilityItem.forecast', { month: month.value }),
    value: data?.forecast ? [{ text: '≈ ' }, money(data.forecast, 0)] : [{ text: '—' }],
  })
  return list
})
const rows = computed(() => (item.value && trend.data.value ? historyRows(item.value, trend.data.value, props.period) : []))

/** A desktop starts in the first reading field; a phone keeps its keyboard down. */
function focusReading(event: Event) {
  event.preventDefault()
  if (props.phone) return
  const target = event.target instanceof HTMLElement ? event.target : null
  target?.querySelector<HTMLInputElement>('[data-utility-reading] input')?.focus()
}

const editOpen = ref(false)
const toggle = useMutation({
  mutationFn: (current: UtilityItem) => {
    const { id, ...rest } = current
    return call<UtilityItem>('update_utility_item', { input: { id, item: { ...rest, active: !current.active } satisfies UtilityInput } })
  },
  onSuccess: () => {
    for (const queryKey of [['utilities'], ['utility-bill'], ...PLANNING_KEYS]) void queryClient.invalidateQueries({ queryKey })
    open.value = false
  },
  meta: { error: 'utilities.saveFailed', success: 'utilities.saved' },
})
</script>

<template>
  <Sheet v-model:open="open">
    <SheetContent side="right" :hide-close="props.phone" class="flex w-full flex-col gap-0 p-0 sm:max-w-[560px]" data-utility-item @open-auto-focus="focusReading">
      <template v-if="item">
        <header :class="cn('flex shrink-0 items-start gap-1', props.phone ? 'h-14 items-center px-2' : 'px-5 pt-4 pb-3 pe-12')">
          <Button v-if="props.phone" variant="ghost" size="icon" class="touch-hit" :aria-label="t('actions.close')" @click="open = false">
            <ChevronLeft class="size-5" />
          </Button>
          <div class="flex min-w-0 flex-col gap-0.5">
            <SheetTitle :class="cn('truncate font-semibold', props.phone ? 'text-xl' : 'text-lg')">{{ item.name }}</SheetTitle>
            <SheetDescription :class="cn('text-muted-foreground', props.phone ? 'sr-only' : 'text-sm')">{{ itemSubtitle(item, t) }}</SheetDescription>
          </div>
        </header>

        <div class="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto overscroll-contain p-5 max-md:p-4">
          <div v-if="meter?.formula" class="flex flex-col gap-1.5 rounded-xl bg-muted p-3" data-utility-formula>
            <span class="text-xs text-muted-foreground">{{ t('utilityItem.byFormula') }}</span>
            <code class="font-mono text-sm font-medium">{{ prettyFormula(meter.formula) }}</code>
            <Button variant="link" size="sm" class="h-auto self-start p-0 text-xs touch-hit" :disabled="props.disabled" @click="editOpen = true">
              {{ t('utilityItem.editFormula') }}
            </Button>
          </div>
          <div :class="cn('grid gap-3', stats.length === 3 ? 'grid-cols-3' : props.phone ? 'grid-cols-1' : 'grid-cols-2')" data-utility-stats>
            <div v-for="stat in stats" :key="stat.label" class="flex flex-col gap-0.5">
              <span class="text-xs text-muted-foreground">{{ stat.label }}</span>
              <span class="text-sm font-semibold tabular-nums"><MoneyPhrase :parts="stat.value" /></span>
            </div>
          </div>
          <UtilityReadingCard v-if="meter && props.line && !props.disabled" :line="props.line" :phone="props.phone" :disabled="props.disabled" />
          <SectionState :loading="trend.isPending.value" :error="trend.error.value" :empty="false" @retry="trend.refetch()">
            <UtilityItemHistory v-if="rows.length" :item="item" :rows="rows" :phone="props.phone" />
          </SectionState>
        </div>

        <footer class="flex shrink-0 items-center gap-2 border-t px-5 pt-3 pb-[max(0.75rem,var(--safe-bottom,0px))] max-md:px-4">
          <Button variant="outline" :class="props.phone && 'h-11 min-w-0 flex-1'" :disabled="props.disabled" @click="editOpen = true">
            <Pencil class="size-4" />{{ t('utilityItem.edit') }}
          </Button>
          <Button
            variant="outline"
            :class="props.phone ? 'h-11 min-w-0 flex-1' : 'ms-auto'"
            :disabled="props.disabled || toggle.isPending.value"
            @click="toggle.mutate(item)"
          >
            {{ item.active ? t('utilityItem.deactivate') : t('utilityItem.activate') }}
          </Button>
        </footer>
        <UtilityDialog v-model:open="editOpen" :item="item" :property-id="item.property_id" />
      </template>
    </SheetContent>
  </Sheet>
</template>
