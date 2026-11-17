<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { AlertTriangle } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { formatDate } from '@/shared/lib/format'

import { lineKey } from './store-change.line-key'

/**
 * The balance corrections a merge shifts: the backend already worked out each
 * line (`CorrectionShift`), this only shows it. Renders nothing when empty.
 */
const props = defineProps<{
  corrections: CorrectionShift[]
  /** The merged account (the one that disappears) and the one it merges into. */
  source: { id: string; name: string }
  targetName: string
}>()

const { t } = useI18n()

const otherName = (shift: CorrectionShift) => (shift.from_store_id === props.source.id ? props.targetName : props.source.name)
</script>

<template>
  <div v-if="props.corrections.length" class="grid gap-1" data-testid="store-change-corrections">
    <p class="flex items-center gap-1.5 text-xs font-medium text-warning">
      <AlertTriangle class="size-3.5" />
      {{ t('storeChange.corrections', { count: props.corrections.length }) }}
    </p>
    <ul class="grid max-h-40 gap-1 overflow-y-auto text-xs">
      <li v-for="shift in props.corrections" :key="lineKey(shift)" class="rounded-md bg-muted px-2 py-1">
        <i18n-t keypath="storeChange.correctionLine">
          <template #wallet>{{ shift.from_store_name }}</template>
          <template #other>{{ otherName(shift) }}</template>
          <template #date>{{ formatDate(shift.timestamp) }}</template>
          <template #was><Money :amount="shift.after_old" :currency="shift.asset.symbol" /></template>
          <template #now><Money :amount="shift.after_new" :currency="shift.asset.symbol" /></template>
          <template #held><Money :amount="shift.other_balance" :currency="shift.asset.symbol" /></template>
        </i18n-t>
      </li>
    </ul>
  </div>
</template>
