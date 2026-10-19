<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { ArrowLeftRight } from 'lucide-vue-next'

import { CurrencyPicker, Money } from '@/entities/currency'
import { AmountInput } from '@/shared/ui/amount-input'
import { Button } from '@/shared/ui/button'
import { DateTimePicker } from '@/shared/ui/date-time-picker'
import { Label } from '@/shared/ui/label'
import { call } from '@/shared/api/ipc'

/** A ↔ B at today's rate or a chosen date. `from` starts it on a currency (a wallet's own). */
const props = withDefaults(defineProps<{ from?: string | null }>(), { from: null })
const { t } = useI18n()

const table = useQuery({ queryKey: ['fx-table'], queryFn: () => call<FxTable>('get_fx_table') })
const codes = computed(() => [table.data.value?.base ?? 'usd', ...(table.data.value?.rows ?? []).map((row) => row.currency)])

const from = ref('')
const to = ref('')
const amount = ref('100')
const date = ref('')
watch(
  [() => table.data.value, () => props.from],
  ([data, start]) => {
    if (!data) return
    from.value = start ?? from.value ?? data.base
    if (!from.value) from.value = data.base
    if (!to.value || to.value === from.value) {
      to.value =
        (from.value === data.base ? data.rows.find((row) => row.tracked)?.currency : data.base) ??
        data.rows[0]?.currency ??
        'usd'
    }
  },
  { immediate: true },
)
const converted = useQuery({
  queryKey: computed(() => ['convert', from.value, to.value, amount.value, date.value]),
  queryFn: () =>
    call<ConvertedAmount>('convert_amount', {
      input: {
        from: from.value,
        to: to.value,
        amount: amount.value.trim() || '0',
        at: date.value ? new Date(`${date.value}T12:00`).toISOString() : null,
      },
    }),
  enabled: computed(() => Boolean(from.value && to.value && Number(amount.value) > 0)),
})
function swap() {
  ;[from.value, to.value] = [to.value, from.value]
}
</script>

<template>
  <div class="grid gap-3">
    <div class="grid grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] items-end gap-2">
      <div class="grid gap-1.5">
        <Label for="convert-from">{{ t('rates.from') }}</Label>
        <CurrencyPicker id="convert-from" v-model="from" label="" compact :extra="codes" />
      </div>
      <Button variant="ghost" size="icon" class="size-9" :aria-label="t('rates.swap')" @click="swap">
        <ArrowLeftRight class="size-4" />
      </Button>
      <div class="grid gap-1.5">
        <Label for="convert-to">{{ t('rates.to') }}</Label>
        <CurrencyPicker id="convert-to" v-model="to" label="" compact :extra="codes" />
      </div>
    </div>
    <div class="grid grid-cols-2 gap-2 [&>*]:min-w-0">
      <div class="grid gap-1.5">
        <Label for="convert-amount">{{ t('rates.amount') }}</Label>
        <AmountInput id="convert-amount" v-model="amount" />
      </div>
      <div class="grid gap-1.5">
        <Label for="convert-date">{{ t('rates.date') }}</Label>
        <DateTimePicker id="convert-date" v-model="date" />
      </div>
    </div>
    <p class="figure">
      <Money v-if="converted.data.value?.amount" :amount="converted.data.value.amount" :currency="to" />
      <template v-else>—</template>
    </p>
    <p v-if="converted.data.value && converted.data.value.amount === null" class="text-xs text-muted-foreground">
      {{ t('rates.noRate') }}
    </p>
  </div>
</template>
