<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { keepPreviousData, useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'

import { Money } from '@/entities/currency'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { AmountInput } from '@/shared/ui/amount-input'
import { DateTimePicker } from '@/shared/ui/date-time-picker'
import { call } from '@/shared/api/ipc'
import { formatMonthKey, formatQuantity } from '@/shared/lib/format'
import { positiveAmount, useFormValidation } from '@/shared/lib/form'
import { localDate, readingDateParts } from '@/shared/lib/planning'
import { cn } from '@/shared/lib/utils'

import { fillFormula, prettyFormula } from '@/shared/lib/utility-formula'

/**
 * «Показание за октябрь» (Wallets-2 · 11–12): a field per tariff zone (or one),
 * the day it was read, and what it comes to before it is saved — «Расход
 * 125 + 70 кВт·ч → 1 036,50 ₽», or the formula with the numbers in it.
 */
const props = defineProps<{ line: UtilityBillLine; phone: boolean; disabled: boolean }>()
const { t } = useI18n()
const queryClient = useQueryClient()

const meter = computed(() => ('metered' in props.line.item.kind ? props.line.item.kind.metered : null))
const zones = computed(() => meter.value?.zones ?? [])
/** The last known reading of each field: the counter starts from it. */
const previous = computed(() =>
  zones.value.length
    ? zones.value.map((zone) => {
        const own = props.line.zones.find((entry) => entry.zone === zone)
        return own?.reading ?? own?.previous_reading ?? '0'
      })
    : [props.line.reading ?? props.line.previous_reading ?? '0'],
)
const values = ref<string[]>([])
const date = ref(localDate(new Date()))
watch(
  () => props.line.item.id,
  () => {
    values.value = previous.value.map(() => '')
    date.value = localDate(new Date())
  },
  { immediate: true },
)
const parts = computed(() => readingDateParts(date.value))
const month = computed(() => formatMonthKey(parts.value?.period ?? localDate(new Date()).slice(0, 7), { month: 'long' }))

const form = useFormValidation((): Record<string, string | null> => ({
  date: parts.value ? null : t('validation.date'),
  ...Object.fromEntries(values.value.map((value, index) => [`v${index}`, positiveAmount(value)])),
}))

const typed = computed(() => values.value.every((value) => Number(value.replace(',', '.')) > 0))
const input = computed<UtilityFormulaInput | null>(() =>
  meter.value && typed.value
    ? {
        tariff: meter.value.tariff,
        formula: meter.value.formula,
        zones: zones.value,
        readings: values.value.map((value, index) => ({
          zone: zones.value[index] ?? null,
          previous: previous.value[index] ?? '0',
          current: value.replace(',', '.'),
        })),
        days: null,
      }
    : null,
)
const preview = useQuery({
  queryKey: computed(() => ['utility-preview', props.line.item.id, input.value]),
  queryFn: () => call<UtilityFormulaPreview>('preview_utility_formula', { input: input.value }),
  enabled: computed(() => input.value !== null),
  placeholderData: keepPreviousData,
})
const used = computed(() =>
  values.value.map((value, index) => Math.max(0, Number(value.replace(',', '.')) - Number(previous.value[index] ?? 0))),
)
/** «Расход 125 + 70 кВт·ч → 1 036,50 ₽»; with a formula «(209 − 205) × 3,5 × 10 = 140,00 ₽». */
const calc = computed(() => {
  const amount = (input.value && preview.data.value?.amount) || null
  if (!meter.value || !amount) return null
  if (meter.value.formula && !zones.value.length) {
    const filled = fillFormula(prettyFormula(meter.value.formula), {
      current: formatQuantity(values.value[0] ?? '0', 3),
      previous: formatQuantity(previous.value[0] ?? '0', 3),
      used: formatQuantity(String(used.value[0] ?? 0), 3),
      tariff: formatQuantity(meter.value.tariff, 3),
    })
    return { text: `${filled} =`, amount }
  }
  const sum = used.value.map((value) => formatQuantity(String(value), 3)).join(' + ')
  return { text: t('utilityItem.consumption', { sum, unit: meter.value.unit }), amount }
})

const save = useMutation({
  mutationFn: async () => {
    for (const [index, value] of values.value.entries())
      await call<MeterReading>('add_meter_reading', {
        input: {
          item_id: props.line.item.id,
          value: value.trim().replace(',', '.'),
          period: parts.value?.period ?? '',
          day: parts.value?.day ?? null,
          zone: zones.value[index] ?? null,
        } satisfies MeterReadingInput,
      })
  },
  onSuccess: () => {
    values.value = values.value.map(() => '')
    form.reset()
    for (const queryKey of [['utility-trend'], ['utility-bill']]) void queryClient.invalidateQueries({ queryKey })
  },
  meta: { error: 'utilities.readingFailed', success: 'utilities.readingSaved' },
})
const label = (index: number) => zones.value[index] ?? t('utilities.reading')
const label1 = (index: number) => label(index).charAt(0).toUpperCase() + label(index).slice(1)
</script>

<template>
  <form
    class="flex flex-col gap-2.5 rounded-xl border border-primary bg-card p-3"
    data-utility-reading
    @submit.prevent="form.submit(save.mutate, $event)"
  >
    <p class="text-sm font-medium">{{ t('utilityItem.readingFor', { month }) }}</p>
    <div :class="cn('grid gap-2.5', values.length > 1 ? (props.phone ? 'grid-cols-2' : 'grid-cols-3') : props.phone ? 'grid-cols-1' : 'grid-cols-2')">
      <div v-for="(_, index) in values" :key="index" class="grid min-w-0 content-start gap-1" :data-invalid="form.invalidAttr(`v${index}`)">
        <span class="text-sm font-medium">{{ label1(index) }} <span class="text-destructive">*</span></span>
        <span class="relative">
          <AmountInput
            v-model="values[index]"
            class="h-9 pe-12"
            :placeholder="formatQuantity(previous[index] ?? '0', 3)"
            :aria-label="label1(index)"
            :disabled="props.disabled"
          />
          <Badge v-if="values.length === 1 && meter" variant="secondary" class="pointer-events-none absolute inset-e-2 top-1/2 -translate-y-1/2 font-normal">
            {{ meter.unit }}
          </Badge>
        </span>
        <span v-if="values.length === 1" class="text-xs text-muted-foreground">{{ t('utilityItem.oneNumber') }}</span>
      </div>
      <div :class="cn('grid min-w-0 content-start gap-1', props.phone && values.length > 1 && 'col-span-full')" :data-invalid="form.invalidAttr('date')">
        <span class="text-sm font-medium">{{ t('utilityItem.readDay') }}</span>
        <DateTimePicker
          v-model="date"
          class="h-9 w-full"
          :max="localDate(new Date())"
          :invalid="Boolean(form.error('date'))"
          :aria-label="t('utilityItem.readDay')"
          :disabled="props.disabled"
        />
      </div>
    </div>
    <p v-if="form.firstMessage()" class="text-xs text-destructive" role="alert">{{ form.firstMessage() }}</p>
    <div class="flex items-center gap-3">
      <span class="min-w-0 flex-1 text-sm text-muted-foreground tabular-nums" data-utility-calc>
        <template v-if="calc">
          {{ calc.text }}
          <b class="font-semibold text-foreground"><Money :amount="calc.amount" :currency="props.line.item.currency" :decimals="2" /></b>
        </template>
        <template v-else>{{ t('utilityItem.recalc') }}</template>
      </span>
      <Button type="submit" :class="props.phone && 'h-11'" :disabled="save.isPending.value">{{ t('utilityItem.save') }}</Button>
    </div>
  </form>
</template>
