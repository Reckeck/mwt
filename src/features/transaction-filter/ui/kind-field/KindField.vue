<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { TX_KINDS } from '@/shared/lib/transactions'
import { Label } from '@/shared/ui/label'
import { Select, SelectItem } from '@/shared/ui/select'

import type { FilterFieldProps } from '../../transaction-filter.types'

/** «Вид»: one kind at a time; several (from a report) show as a chip in the row instead. */
const props = defineProps<FilterFieldProps>()
const filter = defineModel<TxFilter>({ required: true })

const { t } = useI18n()
const kind = computed({
  get: () => (filter.value.kinds.length === 1 ? filter.value.kinds[0] : ''),
  set: (value: string) => {
    filter.value = { ...filter.value, kinds: value ? [value as TxKind] : [] }
  },
})
const id = computed(() => `${props.idPrefix}-kind`)
</script>

<template>
  <div :class="props.bare ? 'contents' : 'grid min-w-0 content-start gap-1.5'">
    <Label v-if="!props.bare" :for="id">{{ t('filter.kind') }}</Label>
    <Select
      :id="id"
      v-model="kind"
      :disabled="filter.kinds.length > 1"
      :aria-label="props.bare ? t('filter.kind') : undefined"
      :class="props.bare ? 'min-w-32 flex-1 basis-32 @3xl:max-w-[170px]' : ''"
    >
      <SelectItem value="">{{ t('filter.allKinds') }}</SelectItem>
      <SelectItem v-for="entry in TX_KINDS" :key="entry" :value="entry">{{ t(`tx.kind.${entry}`) }}</SelectItem>
    </Select>
  </div>
</template>
