<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { formatDateRange } from '@/shared/lib/date-range'
import { FormField } from '@/shared/ui/form-field'
import { Select, SelectItem } from '@/shared/ui/select'

import { useTxForm } from '../../model/use-tx-form'

/**
 * «Событие»: a new payment of mine that belongs to a shared event (the barbecue's meat). Once the
 * payment is saved it is linked to the event as a purchase. Only for a new payment, and only
 * where there is an open event that I take part in.
 */
const { t, locale } = useI18n()
const { sharedEventId, sharedEventOptions } = useTxForm()
const rows = computed(() => sharedEventOptions.value)
const label = (row: SharedEventSummary) => {
  const dates = formatDateRange({ start: row.starts_on, end: row.ends_on }, locale.value)
  return dates ? `${row.title} · ${dates}` : row.title
}
</script>

<template>
  <FormField v-if="rows.length" for="tx-event" :label="t('tx.field.event')" :hint="t('tx.field.eventHint')">
    <Select id="tx-event" v-model="sharedEventId" :aria-label="t('tx.field.event')">
      <SelectItem value="">{{ t('tx.field.noEvent') }}</SelectItem>
      <SelectItem v-for="row in rows" :key="row.id" :value="row.id">{{ label(row) }}</SelectItem>
    </Select>
  </FormField>
</template>
