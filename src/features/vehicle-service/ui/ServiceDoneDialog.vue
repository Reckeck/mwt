<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { integerInRange, required, useFormValidation } from '@/shared/lib/form'
import { localDate } from '@/shared/lib/planning'
import { DateTimePicker } from '@/shared/ui/date-time-picker'
import { FormDialog } from '@/shared/ui/form-dialog'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'

import { useServiceDone } from '../model/use-vehicle'

/** A service done: the next one moves by the car's intervals. */
const props = defineProps<{ store: Store }>()
const open = defineModel<boolean>('open', { required: true })
const { t } = useI18n()

const date = ref('')
const km = ref('')
watch(
  open,
  () => {
    date.value = localDate(new Date())
    km.value = String(props.store.property?.car?.mileage_km ?? '')
  },
  { immediate: true },
)

const form = useFormValidation(
  () => ({ date: required(date.value), km: integerInRange(km.value, 0, 5_000_000) }),
  { resetOn: open },
)
const save = useServiceDone()

function submit() {
  save.mutate(
    { store_id: props.store.id, date: date.value, km: km.value.trim() ? Number(km.value.trim()) : null },
    { onSuccess: () => (open.value = false) },
  )
}
</script>

<template>
  <FormDialog overlay="vehicle-service"
    v-model:open="open"
    :title="t('car.serviceDone')"
    :description="t('car.serviceDoneHint')"
    form-id="service-form"
    :pending="save.isPending.value"
    @submit="form.submit(submit, $event)"
  >
    <FormField for="service-date" :label="t('car.serviceDate')" required :error="form.error('date')">
      <DateTimePicker id="service-date" v-model="date" />
    </FormField>
    <FormField for="service-km" :label="t('car.mileage')" :error="form.error('km')">
      <Input id="service-km" v-model="km" inputmode="numeric" />
    </FormField>
  </FormDialog>
</template>
