<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'

import type { FilterFieldProps } from '../../transaction-filter.types'

/**
 * «Сумма»: from and to in one frame, in the base currency by the operation's
 * larger side. A comma reads as the decimal point; anything else is left out.
 */
const props = defineProps<FilterFieldProps>()
const filter = defineModel<TxFilter>({ required: true })

const { t } = useI18n()

const bound = (name: 'amount_min' | 'amount_max') =>
  computed({
    get: () => filter.value[name] ?? '',
    set: (value: string) => {
      const clean = value.replace(',', '.').replace(/[^\d.]/g, '')
      filter.value = { ...filter.value, [name]: clean && Number.isFinite(Number(clean)) ? clean : null }
    },
  })
const min = bound('amount_min')
const max = bound('amount_max')
const input = 'h-full w-0 min-w-0 flex-1 rounded-none border-0 bg-transparent px-3 shadow-none focus-visible:ring-0 dark:bg-transparent'
</script>

<template>
  <div class="grid min-w-0 content-start gap-1.5">
    <Label :for="`${props.idPrefix}-min`">{{ t('filter.amount') }}</Label>
    <div
      class="touch-hit flex h-9 w-full min-w-0 items-center rounded-md border border-input shadow-sm focus-within:ring-1 focus-within:ring-ring"
    >
      <Input
        :id="`${props.idPrefix}-min`"
        :model-value="min"
        inputmode="decimal"
        :class="input"
        :placeholder="t('filter.amountFrom')"
        @change="(event: Event) => (min = (event.target as HTMLInputElement).value)"
      />
      <span class="text-muted-foreground" aria-hidden="true">–</span>
      <Input
        :id="`${props.idPrefix}-max`"
        :model-value="max"
        @change="(event: Event) => (max = (event.target as HTMLInputElement).value)"
        inputmode="decimal"
        :class="input"
        :placeholder="t('filter.amountTo')"
        :aria-label="t('filter.amountTo')"
      />
    </div>
  </div>
</template>
