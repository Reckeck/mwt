<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { DateTimePicker } from '@/shared/ui/date-time-picker'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'

import type { CarDraft, CarField } from '../../model/car-draft'

/** A car's papers and service plan; the mileage has its own action on the wallet page. */
defineProps<{ error: (field: CarField) => string | undefined }>()
const draft = defineModel<CarDraft>({ required: true })
const { t } = useI18n()
</script>

<template>
  <div class="grid gap-4" data-tour="property-car">
    <div class="grid gap-4 @mid:grid-cols-3">
      <FormField for="car-model" :label="t('car.model')">
        <Input id="car-model" v-model="draft.model" :placeholder="t('car.modelHint')" />
      </FormField>
      <FormField for="car-year" :label="t('car.year')" :error="error('carYear')">
        <Input id="car-year" v-model="draft.year" inputmode="numeric" />
      </FormField>
      <FormField for="car-plate" :label="t('car.plate')">
        <Input id="car-plate" v-model="draft.plate" />
      </FormField>
    </div>
    <div class="grid gap-4 @mid:grid-cols-2">
      <FormField for="car-osago" :label="t('car.osagoUntil')">
        <DateTimePicker id="car-osago" v-model="draft.osagoUntil" />
      </FormField>
      <FormField for="car-kasko" :label="t('car.kaskoUntil')">
        <DateTimePicker id="car-kasko" v-model="draft.kaskoUntil" />
      </FormField>
    </div>
    <p class="text-sm text-muted-foreground">{{ t('car.serviceHint') }}</p>
    <div class="grid gap-4 @mid:grid-cols-2">
      <FormField for="car-service-at" :label="t('car.serviceDueAt')">
        <DateTimePicker id="car-service-at" v-model="draft.serviceDueAt" />
      </FormField>
      <FormField for="car-service-km" :label="t('car.serviceDueKm')" :error="error('carServiceKm')">
        <Input id="car-service-km" v-model="draft.serviceDueKm" inputmode="numeric" />
      </FormField>
      <FormField for="car-interval-months" :label="t('car.intervalMonths')" :error="error('carIntervalMonths')">
        <Input id="car-interval-months" v-model="draft.serviceIntervalMonths" inputmode="numeric" />
      </FormField>
      <FormField for="car-interval-km" :label="t('car.intervalKm')" :error="error('carIntervalKm')">
        <Input id="car-interval-km" v-model="draft.serviceIntervalKm" inputmode="numeric" />
      </FormField>
    </div>
  </div>
</template>
