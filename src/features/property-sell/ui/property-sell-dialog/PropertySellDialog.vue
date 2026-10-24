<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { StoreNodeSelect } from '@/entities/store'
import { AmountInput } from '@/shared/ui/amount-input'
import { Badge } from '@/shared/ui/badge'
import { DateTimePicker } from '@/shared/ui/date-time-picker'
import { FormDialog } from '@/shared/ui/form-dialog'
import { FormField } from '@/shared/ui/form-field'
import { SwitchRow } from '@/shared/ui/switch-row'
import { formatMoney } from '@/shared/lib/format'
import { localDate } from '@/shared/lib/planning'
import { cn } from '@/shared/lib/utils'

import { usePropertySell } from '../../model/use-property-sell'

/**
 * «Продать «Машину»» (Wallets-4 · 21): only a property has it. The sum on a
 * line of its own, the wallet and the day under it at one height, what the
 * thing stood at in the books and the gain over it, and «в архив».
 */
const props = defineProps<{ store: Store; positions: StorePosition[] }>()
const open = defineModel<boolean>('open', { required: true })
const { t } = useI18n()

const { nodes, held, currency, received, proceedsTo, date, archive, sale, gain, form, save } = usePropertySell(
  () => props.store,
  () => props.positions,
  open,
)
const money = (value: string) => formatMoney(value, { currency: currency.value, decimals: Number(value) % 1 ? 2 : 0 })
const signed = computed(() => (gain.value === null ? '—' : `${Number(gain.value) > 0 ? '+' : ''}${money(gain.value)}`))
</script>

<template>
  <FormDialog
    v-model:open="open"
    :title="t('propertySell.title', { name: props.store.name })"
    size="form"
    form-id="property-sell-form"
    :submit-label="t('propertySell.submit')"
    :pending="save.isPending.value"
    phone-submit-only
    @submit="form.submit(save.mutate, $event)"
  >
    <p v-if="!held" class="rounded-xl bg-muted px-3 py-2.5 text-[0.8125rem]" role="alert" data-sell-nothing>
      {{ t('propertySell.nothing') }}
    </p>
    <FormField for="sell-received" :label="t('propertySell.received')" required :error="form.error('received')">
      <AmountInput id="sell-received" v-model="received" class="h-13 [&_input]:text-[1.375rem] [&_input]:font-semibold">
        <template #suffix><Badge variant="secondary" class="me-3">{{ currency.toUpperCase() }}</Badge></template>
      </AmountInput>
    </FormField>
    <div class="grid gap-3.5 @md:grid-cols-2">
      <FormField for="sell-to" :label="t('propertySell.to')" required :error="form.error('proceedsTo')">
        <StoreNodeSelect id="sell-to" v-model="proceedsTo" flat :nodes="nodes" :placeholder="t('propertySell.pickWallet')" :invalid="Boolean(form.error('proceedsTo'))" />
      </FormField>
      <FormField for="sell-date" :label="t('propertySell.date')" :error="form.error('date')">
        <DateTimePicker id="sell-date" v-model="date" class="w-full" :max="localDate(new Date())" />
      </FormField>
    </div>
    <div class="flex flex-col text-sm" data-sell-figures>
      <div class="flex justify-between gap-3 py-1.5">
        <span class="text-muted-foreground">{{ t('propertySell.bookValue') }}</span>
        <span class="tabular-nums">{{ sale ? money(sale.book_value) : '—' }}</span>
      </div>
      <div class="flex justify-between gap-3 py-1.5">
        <span class="text-muted-foreground">{{ t('propertySell.gain') }}</span>
        <span :class="cn('font-medium tabular-nums', gain !== null && Number(gain) > 0 && 'text-success', gain !== null && Number(gain) < 0 && 'text-destructive')" data-sell-gain>
          {{ signed }}
        </span>
      </div>
    </div>
    <SwitchRow id="sell-archive" v-model="archive" plain :label="t('propertySell.archive', { name: props.store.name })" />
  </FormDialog>
</template>
