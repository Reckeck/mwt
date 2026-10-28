<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Link2, X } from 'lucide-vue-next'

import { useIsPhone } from '@/shared/lib/phone'
import { Button } from '@/shared/ui/button'

import { ROW_CHIP_KEYS, useFilterChips } from '../../lib/filter-chips'

/**
 * The set filters as chips, each takes itself off. A phone (Ops-1, «С файлами ×»): every one of them
 * in a line under the search, and «Сбросить» after them. A computer: only those without a field in the
 * row (several kinds from a report, «С комиссией», a chain); the rest are in the row or under «Ещё фильтры».
 */
const props = withDefaults(defineProps<{ fixedScope?: boolean }>(), { fixedScope: false })
const filter = defineModel<TxFilter>({ required: true })
const emit = defineEmits<{ reset: [] }>()

const { t } = useI18n()
const phone = useIsPhone()
const chipsOf = useFilterChips()
const chips = computed(() => {
  const all = chipsOf(filter.value, props.fixedScope)
  if (phone.value) return all
  // One kind is the row's «Вид» field.
  return all.filter((chip) => ROW_CHIP_KEYS.includes(chip.key) && (chip.key !== 'kinds' || filter.value.kinds.length > 1))
})
</script>

<template>
  <div v-if="phone && chips.length" class="flex flex-wrap items-center gap-2" data-filter-chips>
    <Button
      v-for="chip in chips"
      :key="chip.key"
      class="touch-min max-w-full"
      :aria-label="t('filter.chipRemove', { name: chip.label })"
      @click="filter = { ...filter, ...chip.clear }"
    >
      <span class="truncate">{{ chip.label }}</span>
      <X class="size-4" aria-hidden="true" />
    </Button>
    <Button variant="link" class="touch-min px-3" data-filter-chips-reset @click="emit('reset')">{{ t('period.reset') }}</Button>
  </div>
  <template v-else-if="!phone">
    <Button
      v-for="chip in chips"
      :key="chip.key"
      variant="secondary"
      class="shrink-0"
      :title="chip.key === 'flow' ? t('filter.clearFlow') : undefined"
      @click="filter = { ...filter, ...chip.clear }"
    >
      <Link2 v-if="chip.key === 'flow'" class="size-4" />
      {{ chip.label }}
      <X class="size-3.5" />
    </Button>
  </template>
</template>
