<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { Money } from '@/entities/currency'
import { call } from '@/shared/api/ipc'
import { decimalCmp } from '@/shared/lib/decimal'
import { AmountInput } from '@/shared/ui/amount-input'
import { FormField } from '@/shared/ui/form-field'

import { exchangeLoss, exchangeRate } from '../lib/exchange-math'

/**
 * The «got» side of an exchange under the given amount: what came in, the deal rate and what the
 * deal cost over the market rate (or saved). The given amount and the two wallets are the form's.
 */
const props = defineProps<{ gave: string; from: string; to: string; error?: string; invalid?: boolean }>()
const got = defineModel<string>({ default: '' })
const { t } = useI18n()

const rate = computed(() => exchangeRate(props.gave, got.value))
const market = useQuery({
  queryKey: computed(() => ['convert', props.from, props.to, props.gave, '']),
  placeholderData: keepPreviousData,
  queryFn: () => call<ConvertedAmount>('convert_amount', { input: { from: props.from, to: props.to, amount: props.gave, at: null } }),
  enabled: computed(() => Boolean(props.from && props.to && props.from !== props.to && rate.value)),
})
const loss = computed(() => exchangeLoss(market.data.value?.amount ?? null, got.value))
const lossSign = computed(() => (loss.value === null ? 0 : (decimalCmp(loss.value, '0') ?? 0)))
const lossAbs = computed(() => (loss.value ?? '').replace('-', ''))
</script>

<template>
  <div class="grid gap-3" data-exchange>
    <FormField for="quick-got" :label="t('quickAdd.got')" required :error="props.error">
      <AmountInput id="quick-got" v-model="got" :aria-invalid="props.invalid || undefined" :aria-label="t('quickAdd.got')" :calculator="false" />
    </FormField>
    <dl v-if="rate" class="grid gap-1 text-sm" data-exchange-summary>
      <div class="flex items-baseline justify-between gap-3">
        <dt class="text-muted-foreground">{{ t('tx.quick.dealRate') }}</dt>
        <dd class="figure">1 {{ props.from.toUpperCase() }} = <Money :amount="rate" :currency="props.to" /></dd>
      </div>
      <div v-if="loss !== null && lossSign !== 0" class="flex items-baseline justify-between gap-3" data-exchange-loss>
        <dt class="text-muted-foreground">{{ lossSign > 0 ? t('tx.quick.overpay') : t('tx.quick.underpay') }}</dt>
        <dd :class="['figure', lossSign > 0 ? 'text-destructive' : 'text-success']"><Money :amount="lossAbs" :currency="props.to" /></dd>
      </div>
    </dl>
  </div>
</template>
