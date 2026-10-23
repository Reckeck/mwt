<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { useIsPhone } from '@/shared/lib/phone'
import { Label } from '@/shared/ui/label'
import { Switch } from '@/shared/ui/switch'
import { SwitchRow } from '@/shared/ui/switch-row'

import type { FilterFieldProps } from '../../transaction-filter.types'

/**
 * The on/off filters: made by an automation, lent or borrowed, without inner
 * moves. A computer puts the switch before its name, in one line; a phone
 * puts it on the right, a row each.
 */
const props = defineProps<FilterFieldProps>()
const filter = defineModel<TxFilter>({ required: true })

const { t } = useI18n()
const phone = useIsPhone()

const flag = (name: 'auto_only' | 'debt_only' | 'hide_internal' | 'with_attachments') =>
  computed({
    get: () => filter.value[name],
    set: (value: boolean) => {
      filter.value = { ...filter.value, [name]: value }
    },
  })
const rows = [
  { key: 'auto', label: 'filter.autoOnly', model: flag('auto_only') },
  { key: 'debt', label: 'filter.debtOnly', model: flag('debt_only') },
  { key: 'files', label: 'filter.withAttachments', model: flag('with_attachments') },
  { key: 'internal', label: 'filter.hideInternal', model: flag('hide_internal') },
]
</script>

<template>
  <template v-for="row in rows" :key="row.key">
    <SwitchRow v-if="phone" :id="`${props.idPrefix}-${row.key}`" v-model="row.model.value" :label="t(row.label)" plain />
    <div v-else class="flex items-center gap-2">
      <Switch :id="`${props.idPrefix}-${row.key}`" v-model="row.model.value" />
      <Label :for="`${props.idPrefix}-${row.key}`" class="font-normal">{{ t(row.label) }}</Label>
    </div>
  </template>
</template>
