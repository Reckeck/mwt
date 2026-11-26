<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { SwitchRow } from '@/shared/ui/switch-row'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'

import { FILTER_KINDS, FILTER_SWITCHES } from '../../transaction-filter.constants'

/** What the filter sheet edits: kinds, an amount range and the on/off switches. */
defineProps<{ idPrefix: string }>()
const draft = defineModel<TxFilter>({ required: true })

const { t } = useI18n()

function setKinds(kinds: unknown) {
  draft.value = { ...draft.value, kinds: (Array.isArray(kinds) ? kinds : []) as TxKind[] }
}
function setAmount(key: 'amount_min' | 'amount_max', value: string | number) {
  draft.value = { ...draft.value, [key]: String(value).trim() || null }
}
</script>

<template>
  <div class="grid content-start gap-1.5">
    <Label>{{ t('filter.kind') }}</Label>
    <ToggleGroup type="multiple" variant="chip" class="flex-wrap" :model-value="draft.kinds" @update:model-value="setKinds">
      <ToggleGroupItem v-for="kind in FILTER_KINDS" :key="kind" :value="kind">{{ t(`tx.kind.${kind}`) }}</ToggleGroupItem>
    </ToggleGroup>
  </div>
  <div class="grid content-start gap-1.5">
    <Label :for="`${idPrefix}-min`">{{ t('filter.amount') }}</Label>
    <div class="flex items-center gap-2">
      <Input :id="`${idPrefix}-min`" inputmode="decimal" :model-value="draft.amount_min ?? ''" :placeholder="t('filter.amountFrom')" @update:model-value="setAmount('amount_min', $event)" />
      <Input :id="`${idPrefix}-max`" inputmode="decimal" :model-value="draft.amount_max ?? ''" :placeholder="t('filter.amountTo')" :aria-label="t('filter.amountTo')" @update:model-value="setAmount('amount_max', $event)" />
    </div>
  </div>
  <div class="grid gap-2 sm:col-span-2 @4xl:col-span-4">
    <SwitchRow
      v-for="item in FILTER_SWITCHES"
      :id="`${idPrefix}-${item.field}`"
      :key="item.field"
      :label="t(item.label)"
      :model-value="draft[item.field]"
      @update:model-value="draft = { ...draft, [item.field]: $event }"
    />
  </div>
</template>
