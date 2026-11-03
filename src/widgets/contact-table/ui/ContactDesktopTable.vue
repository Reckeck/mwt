<script setup lang="ts">
import { computed, h } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { type ColumnDef, DataTable } from '@/shared/ui/data-table'
import { formatDayMonthShort } from '@/shared/lib/format'

import type { ContactRow } from '../contact-table.types'
import ContactDebtCell from './ContactDebtCell.vue'
import ContactNameCell from './ContactNameCell.vue'

/** The contacts as a table: name, type, group, debt, last operation, requisites. Sorts by any column. */
const props = defineProps<{ rows: ContactRow[]; selectedId: string }>()
const emit = defineEmits<{ select: [id: string] }>()

const { t } = useI18n()

const columns = computed<ColumnDef<ContactRow>[]>(() => [
  {
    id: 'name',
    header: t('contacts.table.name'),
    accessorFn: (row) => row.contact.name.toLowerCase(),
    cell: ({ row }) => h(ContactNameCell, { contact: row.original.contact, selected: row.original.id === props.selectedId }),
  },
  {
    id: 'kind',
    header: t('contacts.kind'),
    accessorFn: (row) => t(`contactKind.${row.contact.kind}`),
    meta: { class: 'text-muted-foreground' },
  },
  {
    id: 'group',
    header: t('contacts.group'),
    accessorFn: (row) => row.contact.group.trim(),
    cell: ({ row }) => row.original.contact.group.trim() || '—',
    meta: { class: 'text-muted-foreground' },
  },
  {
    id: 'debt',
    header: t('contacts.table.debt'),
    accessorFn: (row) => row.debtBase,
    cell: ({ row }) => h(ContactDebtCell, { nets: row.original.nets }),
    meta: { align: 'right' },
  },
  {
    id: 'last',
    header: t('contacts.table.lastOperation'),
    accessorFn: (row) => row.lastAt ?? '',
    cell: ({ row }) =>
      row.original.lastAt
        ? h('span', { class: 'inline-flex items-baseline gap-2 whitespace-nowrap' }, [
            h('span', formatDayMonthShort(row.original.lastAt)),
            row.original.lastAmount && row.original.lastAsset
              ? h('span', { class: 'text-xs text-muted-foreground' }, [
                  h(Money, { amount: row.original.lastAmount, currency: row.original.lastAsset, signed: true, convert: false }),
                ])
              : null,
          ])
        : '—',
    meta: { class: 'text-muted-foreground' },
  },
  {
    id: 'accounts',
    header: t('contacts.table.requisites'),
    accessorFn: (row) => row.accountCount,
    meta: { align: 'right', class: 'text-muted-foreground' },
  },
])
</script>

<template>
  <DataTable
    :columns="columns"
    :data="props.rows"
    :initial-sorting="[{ id: 'name', desc: false }]"
    :get-row-id="(row) => row.id"
    :row-class="(row) => (row.id === props.selectedId ? 'bg-accent hover:bg-accent' : undefined)"
    clickable
    @row-click="(row) => emit('select', row.id)"
  />
</template>
