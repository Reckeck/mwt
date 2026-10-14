<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { Button } from '@/shared/ui/button'
import { AmountInput } from '@/shared/ui/amount-input'
import { call } from '@/shared/api/ipc'
import { localDate, readingDateParts } from '@/shared/lib/planning'
import { positiveAmount, useFormValidation } from '@/shared/lib/form'
import { DateTimePicker } from '@/shared/ui/date-time-picker'
import { Select } from '@/shared/ui/select'

import type { MeterReadingFormProps } from './meter-reading.types'

const props = defineProps<MeterReadingFormProps>()

const { t } = useI18n()
const queryClient = useQueryClient()

/** The day the meter was read, today by default. */
const date = ref(localDate(new Date()))
const value = ref('')
const parts = computed(() => readingDateParts(date.value))
/** A day/night meter takes a reading per zone. */
const zones = computed(() => ('metered' in props.item.kind ? props.item.kind.metered.zones : []))
const zone = ref('')
watch(zones, (list) => (zone.value = list.includes(zone.value) ? zone.value : (list[0] ?? '')), { immediate: true })
const form = useFormValidation(() => ({
  date: parts.value ? null : t('validation.date'),
  value: positiveAmount(value.value),
}))

const save = useMutation({
  mutationFn: () =>
    call<MeterReading>('add_meter_reading', {
      input: {
        item_id: props.item.id,
        value: value.value.trim(),
        period: parts.value?.period ?? '',
        day: parts.value?.day ?? null,
        zone: zones.value.length ? zone.value : null,
      } satisfies MeterReadingInput,
    }),
  onSuccess: () => {
    value.value = ''
    form.reset()
    void queryClient.invalidateQueries({ queryKey: ['utility-trend'] })
  },
  meta: { error: 'utilities.readingFailed', success: 'utilities.readingSaved' },
})
</script>

<template>
  <form class="grid gap-1" @submit.prevent="form.submit(save.mutate, $event)">
    <div class="flex flex-wrap items-center gap-2">
      <span class="contents" :data-invalid="form.invalidAttr('date')">
        <DateTimePicker
          v-model="date"
          class="h-8 w-40"
          :max="localDate(new Date())"
          :invalid="Boolean(form.error('date'))"
          :aria-label="t('utilities.day')"
          :title="t('utilities.day')"
        />
      </span>
      <Select v-if="zones.length" v-model="zone" class="h-8 w-28" :aria-label="t('utilities.zone')">
        <option v-for="entry in zones" :key="entry" :value="entry">{{ entry }}</option>
      </Select>
      <span class="contents" :data-invalid="form.invalidAttr('value')">
        <AmountInput
          v-model="value"
          class="h-8 w-28"
          :placeholder="t('utilities.reading')"
          :aria-label="t('utilities.reading')"
        />
      </span>
      <Button type="submit" variant="outline" size="sm" :disabled="save.isPending.value">
        {{ t('utilities.saveReading') }}
      </Button>
    </div>
    <p v-if="form.firstMessage()" class="text-xs text-destructive" role="alert">{{ form.firstMessage() }}</p>
  </form>
</template>
