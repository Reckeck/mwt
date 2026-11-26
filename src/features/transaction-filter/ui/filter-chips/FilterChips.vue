<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { X } from 'lucide-vue-next'

import { Badge } from '@/shared/ui/badge'
import { emptyTxFilter } from '@/shared/lib/tx-filter'

import { useFilterChips } from '../../lib/filter-chips'

/** Every set filter but the search as a chip that takes itself off. */
const filter = defineModel<TxFilter>({ required: true })

const { t } = useI18n()
const chipsOf = useFilterChips()
</script>

<template>
  <ul v-if="chipsOf(filter).length" class="flex flex-wrap items-center gap-2" data-filter-chips>
    <li v-for="chip in chipsOf(filter)" :key="chip.key">
      <Badge variant="neutral" class="gap-1 pe-1" data-filter-chip>
        <span class="max-w-48 truncate">{{ chip.label }}</span>
        <button
          type="button"
          class="touch-min-square flex size-5 items-center justify-center rounded-full hover:bg-border"
          :aria-label="t('filter.chipRemove', { name: chip.label })"
          @click="filter = { ...filter, ...chip.clear }"
        >
          <X class="size-3" />
        </button>
      </Badge>
    </li>
    <li v-if="chipsOf(filter).length > 1">
      <button type="button" class="text-xs text-muted-foreground underline-offset-2 hover:underline" @click="filter = emptyTxFilter({ text: filter.text })">
        {{ t('period.reset') }}
      </button>
    </li>
  </ul>
</template>
