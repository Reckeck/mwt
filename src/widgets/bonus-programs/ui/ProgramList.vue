<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChevronRight, Gift, Minus, Plus, Settings2 } from 'lucide-vue-next'

import { Money, MoneyPhrase } from '@/entities/currency'
import { partsText } from '@/shared/lib/money-parts'
import { ActionSheet } from '@/shared/ui/action-menu'
import { Button } from '@/shared/ui/button'
import type { UiAction } from '@/shared/ui/context-menu'
import { cn } from '@/shared/lib/utils'

import type { PointsMode, ProgramRow } from '../bonus-programs.types'

/**
 * The programmes on a phone (Ops-2 · 12): a list of records with the balance and
 * what it comes to, as the desktop table has; a tap brings up what can be done.
 */
const props = defineProps<{ rows: ProgramRow[]; baseCurrency: string; highlighted: string }>()
const emit = defineEmits<{ points: [store: Store, mode: PointsMode]; settings: [store: Store] }>()
const { t } = useI18n()

const picked = ref<ProgramRow | null>(null)
const sheetOpen = computed({
  get: () => picked.value !== null,
  set: (value: boolean) => {
    if (!value) picked.value = null
  },
})
const actions = computed<UiAction[]>(() => {
  const store = picked.value?.store
  if (!store) return []
  return [
    { id: 'credit', label: t('loyalty.credit'), icon: Plus, disabled: !store.loyalty, run: () => emit('points', store, 'credit') },
    { id: 'spend', label: t('loyalty.spend'), icon: Minus, disabled: !store.loyalty, run: () => emit('points', store, 'spend') },
    { id: 'settings', label: t('bonuses.settings'), icon: Settings2, run: () => emit('settings', store) },
  ]
})
</script>

<template>
  <div class="divide-y rounded-xl border border-border bg-card px-3 shadow-sm">
    <Button
      v-for="row in props.rows"
      :key="row.store.id"
      variant="ghost"
      :class="
        cn(
          'h-auto min-h-14 w-full justify-start gap-3 rounded-none px-0 py-2.5 text-left font-normal hover:bg-transparent',
          row.store.id === props.highlighted && 'bg-primary/5',
        )
      "
      :data-program="row.store.id"
      @click="picked = row"
    >
      <span class="flex size-9 shrink-0 items-center justify-center rounded-lg bg-secondary text-muted-foreground">
        <Gift class="size-4" />
      </span>
      <span class="flex min-w-0 flex-1 flex-col">
        <span class="truncate text-sm font-medium">{{ row.store.name }}</span>
        <span :class="cn('truncate text-xs', row.soon ? 'text-warning' : 'text-muted-foreground')">
          <template v-if="row.soon">{{ row.expiring }}</template>
          <template v-else>
            <MoneyPhrase :parts="row.subtitle" />
          </template>
        </span>
      </span>
      <span class="grid shrink-0 justify-items-end gap-0.5">
        <span class="text-sm font-semibold tabular-nums">{{ row.points }}</span>
        <span class="text-xs text-muted-foreground tabular-nums">≈ <Money :amount="row.valueBase" :currency="props.baseCurrency" :decimals="0" /></span>
      </span>
      <ChevronRight class="size-4 shrink-0 text-muted-foreground" />
    </Button>
  </div>
  <ActionSheet
    v-model:open="sheetOpen"
    :actions="actions"
    :title="picked?.store.name ?? ''"
    :description="picked ? `${picked.points} · ${partsText(picked.subtitle)}` : ''"
  />
</template>
