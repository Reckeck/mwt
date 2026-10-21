<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'

import { MileageDialog, ServiceDoneDialog } from '@/features/vehicle-service'
import { formatDate, numberFormat } from '@/shared/lib/format'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'

/** A car at a glance: its papers, the odometer and the next service, with the two everyday actions. */
const props = defineProps<{ store: Store }>()
const { t } = useI18n()

/** Kilometres left before the service is reminded (`CarInfo::SERVICE_KM_AHEAD`). */
const SERVICE_KM_AHEAD = 1000

const car = computed(() => props.store.property?.car ?? null)
const km = (value: number | null | undefined) => (value == null ? '—' : `${numberFormat().format(value)} ${t('car.km')}`)
const kmLeft = computed(() => {
  const due = car.value?.service_due_km
  const now = car.value?.mileage_km
  return due != null && now != null ? due - now : null
})
const serviceSoon = computed(() => kmLeft.value !== null && kmLeft.value < SERVICE_KM_AHEAD)

const mileageOpen = ref(false)
const serviceOpen = ref(false)
</script>

<template>
  <Card data-tour="vehicle-card">
    <template #header>
      <div class="flex flex-wrap items-center justify-between gap-2">
        <div class="min-w-0">
          <p class="truncate font-medium">{{ car?.model || t('car.title') }}</p>
          <p v-if="car?.plate || car?.year" class="text-xs text-muted-foreground">
            {{ [car?.plate, car?.year].filter(Boolean).join(' · ') }}
          </p>
        </div>
        <div class="flex flex-wrap gap-2">
          <Button variant="outline" size="sm" @click="mileageOpen = true">{{ t('car.updateMileage') }}</Button>
          <Button variant="outline" size="sm" @click="serviceOpen = true">{{ t('car.serviceDone') }}</Button>
        </div>
      </div>
    </template>
    <dl class="grid grid-cols-2 gap-x-4 gap-y-1.5 text-sm @mid:grid-cols-4">
      <div class="grid content-start gap-0.5">
        <dt class="text-xs text-muted-foreground">{{ t('car.mileageNow') }}</dt>
        <dd class="font-medium tabular-nums">{{ km(car?.mileage_km) }}</dd>
        <dd v-if="car?.mileage_at" class="text-xs text-muted-foreground">{{ formatDate(car.mileage_at) }}</dd>
      </div>
      <div class="grid content-start gap-0.5">
        <dt class="text-xs text-muted-foreground">{{ t('car.nextService') }}</dt>
        <dd class="font-medium" :class="serviceSoon ? 'text-warning' : ''">
          {{ car?.service_due_at ? formatDate(car.service_due_at) : '—' }}
        </dd>
        <dd v-if="car?.service_due_km != null" class="text-xs tabular-nums text-muted-foreground">
          {{ km(car.service_due_km) }}<template v-if="kmLeft !== null"> · {{ t('car.kmLeft', { km: km(kmLeft) }) }}</template>
        </dd>
      </div>
      <div class="grid content-start gap-0.5">
        <dt class="text-xs text-muted-foreground">{{ t('car.osagoUntil') }}</dt>
        <dd class="font-medium">{{ formatDate(car?.osago_until) }}</dd>
      </div>
      <div class="grid content-start gap-0.5">
        <dt class="text-xs text-muted-foreground">{{ t('car.kaskoUntil') }}</dt>
        <dd class="font-medium">{{ formatDate(car?.kasko_until) }}</dd>
      </div>
    </dl>
    <p v-if="!car" class="mt-3 text-sm text-muted-foreground">{{ t('car.empty') }}</p>
    <MileageDialog v-model:open="mileageOpen" :store="props.store" />
    <ServiceDoneDialog v-model:open="serviceOpen" :store="props.store" />
  </Card>
</template>
