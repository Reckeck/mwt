<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { useIsPhone } from '@/shared/lib/phone'
import { DataTable } from '@/shared/ui/data-table'

import CryptoPositionCard from './CryptoPositionCard.vue'
import type { CryptoPositionTableProps } from './crypto-position-table.types'
import { usePositionActions } from './use-position-actions'
import { usePositionColumns } from './use-position-columns'

/**
 * The positions as a table (desktop) or a list of cards (phone, below `md`): seven columns do not read at 390 px.
 * A click on a row, or the «Open» of its «⋯» menu, opens the detail; «Operations» goes to the list narrowed to
 * the position's `TrackingRef::Position`.
 */
const props = defineProps<CryptoPositionTableProps>()
const emit = defineEmits<{ open: [position: CryptoPositionView] }>()

const { t } = useI18n()
const phone = useIsPhone()
const actionsOf = usePositionActions((position) => emit('open', position))
const columns = usePositionColumns(actionsOf)
// check-ui-ignore fixed-width: the table scrolls sideways inside its own wrapper on a narrow window
</script>

<template>
  <ul v-if="phone" class="grid grid-cols-[minmax(0,1fr)] gap-3" :aria-label="t('cryptoTrack.positions.title')">
    <li v-for="position in props.positions" :key="position.id">
      <CryptoPositionCard :position="position" :actions="actionsOf(position)" @open="emit('open', position)" />
    </li>
  </ul>
  <div v-else class="overflow-x-auto">
    <DataTable
      class="min-w-[52rem]"
      sticky-first-column
      clickable
      :columns="columns"
      :data="props.positions"
      :get-row-id="(position) => position.id"
      :initial-sorting="[{ id: 'value', desc: true }]"
      @row-click="(position) => emit('open', position)"
    />
  </div>
</template>
