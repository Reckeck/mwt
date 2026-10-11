<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { toast } from 'vue-sonner'

import { Button } from '@/shared/ui/button'
import { AmountInput } from '@/shared/ui/amount-input'
import { Input } from '@/shared/ui/input'
import { call, parseError } from '@/shared/api/ipc'
import { localDate } from '@/shared/lib/planning'
import { codeErrorMessage } from '@/shared/lib/vault-error'

import type { MeterReadingFormProps } from './meter-reading.types'

const props = defineProps<MeterReadingFormProps>()

const { t } = useI18n()
const queryClient = useQueryClient()

const period = ref(localDate(new Date()).slice(0, 7))
/** Day of the month; empty = the month's end. */
const day = ref('')
const value = ref('')

const save = useMutation({
  mutationFn: () =>
    call<MeterReading>('add_meter_reading', {
      input: {
        item_id: props.item.id,
        value: value.value.trim(),
        period: period.value,
        day: day.value.trim() ? Number(day.value) : null,
      } satisfies MeterReadingInput,
    }),
  onSuccess: () => {
    toast.success(t('utilities.readingSaved'))
    value.value = ''
    day.value = ''
    void queryClient.invalidateQueries({ queryKey: ['utility-trend'] })
  },
  onError: (err) => toast.error(t('utilities.readingFailed'), { description: codeErrorMessage(parseError(err), t) }),
})
</script>

<template>
  <form class="flex flex-wrap items-center gap-2" @submit.prevent="value.trim() && save.mutate()">
    <Input v-model="period" type="month" class="h-8 w-36" :aria-label="t('utilities.period')" />
    <Input
      v-model="day"
      type="number"
      min="1"
      max="31"
      class="h-8 w-20 px-2"
      :placeholder="t('utilities.dayShort')"
      :aria-label="t('utilities.day')"
      :title="t('utilities.dayHint')"
    />
    <AmountInput
      v-model="value"
      class="h-8 w-28"
      :placeholder="t('utilities.reading')"
      :aria-label="t('utilities.reading')"
    />
    <Button type="submit" variant="outline" size="sm" :disabled="!value.trim() || save.isPending.value">
      {{ t('utilities.saveReading') }}
    </Button>
  </form>
</template>
