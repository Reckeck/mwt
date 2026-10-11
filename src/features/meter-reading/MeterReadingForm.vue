<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { toast } from 'vue-sonner'

import { Button } from '@/shared/ui/button'
import { AmountInput } from '@/shared/ui/amount-input'
import { Input } from '@/shared/ui/input'
import { call, parseError } from '@/shared/api/ipc'
import { localDate, readingDateParts } from '@/shared/lib/planning'
import { codeErrorMessage } from '@/shared/lib/vault-error'

import type { MeterReadingFormProps } from './meter-reading.types'

const props = defineProps<MeterReadingFormProps>()

const { t } = useI18n()
const queryClient = useQueryClient()

/** The day the meter was read, today by default. */
const date = ref(localDate(new Date()))
const value = ref('')
const parts = computed(() => readingDateParts(date.value))

const save = useMutation({
  mutationFn: () =>
    call<MeterReading>('add_meter_reading', {
      input: {
        item_id: props.item.id,
        value: value.value.trim(),
        period: parts.value?.period ?? '',
        day: parts.value?.day ?? null,
      } satisfies MeterReadingInput,
    }),
  onSuccess: () => {
    toast.success(t('utilities.readingSaved'))
    value.value = ''
    void queryClient.invalidateQueries({ queryKey: ['utility-trend'] })
  },
  onError: (err) => toast.error(t('utilities.readingFailed'), { description: codeErrorMessage(parseError(err), t) }),
})
</script>

<template>
  <form class="flex flex-wrap items-center gap-2" @submit.prevent="value.trim() && parts && save.mutate()">
    <Input
      v-model="date"
      type="date"
      class="h-8 w-40"
      :max="localDate(new Date())"
      :aria-label="t('utilities.day')"
      :title="t('utilities.day')"
    />
    <AmountInput
      v-model="value"
      class="h-8 w-28"
      :placeholder="t('utilities.reading')"
      :aria-label="t('utilities.reading')"
    />
    <Button type="submit" variant="outline" size="sm" :disabled="!value.trim() || !parts || save.isPending.value">
      {{ t('utilities.saveReading') }}
    </Button>
  </form>
</template>
