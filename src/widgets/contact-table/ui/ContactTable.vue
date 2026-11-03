<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Plus, UserSearch } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { EmptyState } from '@/shared/ui/empty-state'
import { SectionState } from '@/shared/ui/section-state'
import { Skeleton } from '@/shared/ui/skeleton'
import { useContactSelection } from '@/shared/lib/contact-selection'
import { useIsPhone } from '@/shared/lib/phone'

import type { ContactFilter } from '../contact-table.types'
import { filterRows, useContactRows } from '../model/use-contact-rows'
import ContactDesktopTable from './ContactDesktopTable.vue'
import ContactFilters from './ContactFilters.vue'
import ContactPhoneList from './ContactPhoneList.vue'
import ContactSummaryTiles from './ContactSummaryTiles.vue'

/**
 * The contacts page body: who owes whom in two tiles, the filters, and the list. A row opens the contact's
 * card at the right (`?contact=`); the table keeps its width under it.
 */
const emit = defineEmits<{ create: [] }>()

const { t } = useI18n()
const phone = useIsPhone()
const { selectedId, select } = useContactSelection()
const { rows, totals, counts, loading, error, total } = useContactRows()

const filter = ref<ContactFilter>({ search: '', scope: 'all', direction: null, kind: '' })
/** On a phone the list opens on who owes whom: the largest debts first, the rest by name. */
const shown = computed(() => {
  const found = filterRows(rows.value, filter.value)
  if (!phone.value) return found
  return [...found].sort(
    (left, right) => Math.abs(right.debtBase) - Math.abs(left.debtBase) || left.contact.name.localeCompare(right.contact.name),
  )
})
const filtered = computed(() => Boolean(filter.value.search.trim() || filter.value.direction || filter.value.kind || filter.value.scope !== 'all'))

function reset() {
  filter.value = { search: '', scope: 'all', direction: null, kind: '' }
}
</script>

<template>
  <div class="grid min-w-0 grid-cols-1 gap-4">
    <!-- The action stands next to the tiles, not at the far edge: the open card covers the right side. -->
    <div class="flex flex-wrap items-center gap-3">
      <ContactSummaryTiles v-model:direction="filter.direction" :totals="totals" :loading="loading" class="max-md:w-full" />
      <div v-if="$slots.actions" class="max-md:order-first max-md:flex max-md:w-full max-md:justify-end">
        <slot name="actions" />
      </div>
    </div>
    <ContactFilters v-model:search="filter.search" v-model:scope="filter.scope" v-model:kind="filter.kind" :counts="counts" />

    <div class="min-w-0" data-tour="contact-groups">
      <SectionState :loading="loading" :error="error" skeleton="table" :rows="6">
        <template #loading>
          <div class="grid grid-cols-1 gap-px overflow-hidden rounded-xl border">
            <Skeleton v-for="index in 6" :key="index" class="h-[52px] w-full rounded-none" />
          </div>
        </template>
        <EmptyState
          v-if="!total"
          :icon="UserSearch"
          :title="t('contacts.empty')"
          :description="t('contacts.table.emptyHint')"
        >
          <Button @click="emit('create')"><Plus class="size-4" />{{ t('contacts.add') }}</Button>
        </EmptyState>
        <EmptyState v-else-if="!shown.length" compact :title="t('contacts.nothingFound')">
          <Button v-if="filtered" variant="outline" @click="reset">{{ t('contacts.table.resetFilters') }}</Button>
        </EmptyState>
        <ContactPhoneList v-else-if="phone" :rows="shown" :selected-id="selectedId" @select="select" />
        <div v-else class="[scrollbar-gutter:stable]">
          <ContactDesktopTable :rows="shown" :selected-id="selectedId" @select="select" />
        </div>
      </SectionState>
    </div>
  </div>
</template>
