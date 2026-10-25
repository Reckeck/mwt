<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Info } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { AmountInput } from '@/shared/ui/amount-input'
import { Badge } from '@/shared/ui/badge'
import { FormField } from '@/shared/ui/form-field'

import { difference } from '../../lib/adjustment-rows'
import type { AdjustmentInApp, AdjustmentRow } from '../../store-adjust.types'

/**
 * One currency on cash or a bank account (Wallets-3 · 20): what the app holds,
 * what is really there, and the gap the correction will book.
 */
const props = defineProps<{ row: AdjustmentRow; inApp: AdjustmentInApp | null; cash: boolean; error?: string }>()
const { t } = useI18n()

/** The gap the correction books; text while there is none to show. */
const gap = computed(() => {
  const value = difference(props.row)
  if (value === null) return { text: t('adjustment.simple.typeIt') }
  if (value === '0') return { text: t('adjustment.simple.noGap') }
  return { amount: value }
})
</script>

<template>
  <div class="flex justify-between gap-3 py-1.5 text-sm">
    <span class="text-muted-foreground">{{ t('adjustment.simple.inApp') }}</span>
    <span v-if="props.inApp" class="tabular-nums">
      <template v-if="props.inApp.base"><Money :amount="props.inApp.base.amount" :currency="props.inApp.base.currency" :decimals="2" /> · </template>
      <Money :amount="props.inApp.own.amount" :currency="props.inApp.own.currency" />
    </span>
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
    <i18n-t v-if="gap.amount" keypath="adjustment.simple.gap" tag="span" class="min-w-0 flex-1 tabular-nums">
      <template #amount><Money :amount="gap.amount" :currency="props.row.asset.symbol" signed /></template>
    </i18n-t>
    <span v-else class="min-w-0 flex-1 tabular-nums">{{ gap.text }}</span>
  </p>
</template>
