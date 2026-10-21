<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Trash2 } from 'lucide-vue-next'

import { AmountInput } from '@/shared/ui/amount-input'
import { Button } from '@/shared/ui/button'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'
import { formatMonthKey, formatPct } from '@/shared/lib/format'

import InflationSpark from './InflationSpark.vue'

/**
 * One currency: Auto (the statistics office's latest month) or Manual (the
 * user's flat rate). The rate field stays in place in both modes, so a
 * switch moves nothing; in Auto it shows the fetched value, read-only.
 */
const props = defineProps<{ code: string; entry: InflationCurrency | null; removable: boolean }>()
const manual = defineModel<string>({ required: true })
const emit = defineEmits<{ change: []; mode: [mode: InflationMode]; remove: [] }>()

const { t } = useI18n()
const label = computed(() => props.code.toUpperCase())
const auto = computed(() => props.entry?.mode === 'auto')
const latest = computed(() => props.entry?.latest ?? null)
const delta = computed(() => {
  const previous = props.entry?.previous
  if (!latest.value || previous == null) return null
  return Number(latest.value.yoy_pct) - Number(previous)
})
const shown = computed({
  get: () => (auto.value ? (props.entry?.effective ?? '') : manual.value),
  set: (value: string) => {
    if (!auto.value) manual.value = value
  },
})
</script>

<template>
  <div class="grid gap-1 border-b py-2 last:border-b-0">
    <div class="flex flex-wrap items-center gap-2">
      <span class="w-12 text-sm font-medium">{{ label }}</span>
      <ToggleGroup
        v-if="props.entry?.source"
        type="single"
        size="sm"
        variant="outline"
        :model-value="props.entry.mode"
        :aria-label="t('inflation.modeFor', { currency: label })"
        @update:model-value="(mode) => mode && emit('mode', mode as InflationMode)"
      >
        <ToggleGroupItem value="auto">{{ t('inflation.mode.auto') }}</ToggleGroupItem>
        <ToggleGroupItem value="manual">{{ t('inflation.mode.manual') }}</ToggleGroupItem>
      </ToggleGroup>
      <AmountInput
        v-model="shown"
        class="w-24"
        :disabled="auto"
        :aria-label="t('inflation.rateFor', { currency: label })"
        @change="emit('change')"
      />
      <span class="text-sm text-muted-foreground">{{ t('inflation.perYear') }}</span>
      <InflationSpark v-if="props.entry?.series.length" :series="props.entry.series" class="ml-auto" />
      <Button
        variant="ghost"
        size="icon-sm"
        class="text-muted-foreground"
        :class="[!props.entry?.series.length && 'ml-auto', !props.removable && 'invisible']"
        :aria-label="t('inflation.remove', { currency: label })"
        :aria-hidden="!props.removable"
        :tabindex="props.removable ? 0 : -1"
        @click="emit('remove')"
      >
        <Trash2 class="size-4" />
      </Button>
    </div>
    <p v-if="props.entry?.source" class="text-xs text-muted-foreground">
      {{ t(`inflation.source.${props.entry.source}`) }}
      <template v-if="latest">
        · {{ t('inflation.latest', { month: formatMonthKey(latest.month), value: formatPct(latest.yoy_pct) }) }}
        <template v-if="delta !== null">({{ t('inflation.delta', { value: formatPct(delta, { signed: true }) }) }})</template>
      </template>
      <template v-else> · {{ t('inflation.noReadings') }}</template>
      <template v-if="auto && !latest"> · {{ t('inflation.manualUntilFirst') }}</template>
    </p>
  </div>
</template>
