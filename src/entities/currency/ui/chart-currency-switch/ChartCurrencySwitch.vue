<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'

import type { ChartCurrencySwitchProps } from './chart-currency-switch.types'

/** The currency a money chart is drawn in: the base or a favourite. */
const props = defineProps<ChartCurrencySwitchProps>()
const currency = defineModel<string>({ required: true })
const { t } = useI18n()
</script>

<template>
  <ToggleGroup
    v-if="props.options.length > 1"
    v-model="currency"
    type="single"
    variant="outline"
    size="sm"
    :aria-label="t('charts.currency')"
    data-tour="chart-currency"
  >
    <ToggleGroupItem v-for="code in props.options" :key="code" :value="code" class="px-2 text-xs uppercase">
      {{ code }}
    </ToggleGroupItem>
  </ToggleGroup>
</template>
