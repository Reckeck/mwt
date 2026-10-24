<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Info } from 'lucide-vue-next'

import { AmountInput } from '@/shared/ui/amount-input'
import { Badge } from '@/shared/ui/badge'
import { FormField } from '@/shared/ui/form-field'

import { difference, shownBalance } from '../../lib/adjustment-rows'
import type { AdjustmentRow } from '../../store-adjust.types'

/**
 * One currency on cash or a bank account (Wallets-3 · 20): what the app holds,
 * what is really there, and the gap the correction will book.
 */
const props = defineProps<{ row: AdjustmentRow; inApp: string; cash: boolean; error?: string }>()
const { t } = useI18n()

const gap = computed(() => {
  const value = difference(props.row)
  if (value === null) return t('adjustment.simple.typeIt')
  if (value === '0') return t('adjustment.simple.noGap')
  return t('adjustment.simple.gap', { amount: shownBalance(value, props.row.asset, true) })
})
</script>

<template>
  <div class="flex justify-between gap-3 py-1.5 text-sm">
    <span class="text-muted-foreground">{{ t('adjustment.simple.inApp') }}</span>
    <span class="tabular-nums">{{ props.inApp }}</span>
  </div>
  <FormField for="adjustment-actual" :label="t(props.cash ? 'adjustment.simple.actualCash' : 'adjustment.simple.actualBank')" required :error="props.error">
    <AmountInput
      id="adjustment-actual"
      v-model="props.row.actual"
      :placeholder="props.row.before"
      class="h-13 [&_input]:text-[1.375rem] [&_input]:font-semibold"
    >
      <template #suffix><Badge variant="secondary" class="me-3">{{ props.row.asset.symbol }}</Badge></template>
    </AmountInput>
  </FormField>
  <p class="flex items-center gap-2 rounded-xl bg-muted px-3 py-2.5 text-[0.8125rem]" data-adjustment-gap>
    <Info class="size-4 shrink-0" />
    <span class="min-w-0 flex-1 tabular-nums">{{ gap }}</span>
  </p>
</template>
