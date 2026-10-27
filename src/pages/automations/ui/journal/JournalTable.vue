<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { RouterLink, useRoute } from 'vue-router'

import { MoneyPhrase } from '@/entities/currency'
import { entityLocation } from '@/shared/lib/entity-link'
import { Button } from '@/shared/ui/button'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/shared/ui/table'

import type { JournalRow } from './journal.types'

/**
 * The journal on a computer (Auto-1 · 4): Когда · Автоматизация · Итог · Что
 * сделала; a run waiting for «да» has «Подтвердить» at the end of its row.
 */
defineProps<{ rows: JournalRow[] }>()
const emit = defineEmits<{ confirm: [entry: JournalEntry] }>()
const { t } = useI18n()
const route = useRoute()

const TH = 'h-auto px-3 py-2 text-xs font-medium text-muted-foreground'
const opens = (entry: JournalEntry) => entry.kind === 'scheme' || entry.kind === 'repeat'
</script>

<template>
  <div class="overflow-hidden rounded-xl border border-border bg-card shadow-sm" data-journal-table>
    <!-- Fixed height: a long journal scrolls inside its card, the header stays. -->
    <Table container-class="max-h-[34rem] overscroll-contain">
      <TableHeader class="sticky top-0 z-10 bg-card shadow-[0_1px_0_var(--border)]">
        <TableRow>
          <TableHead :class="TH">{{ t('automations.journal.col.when') }}</TableHead>
          <TableHead :class="TH">{{ t('automations.journal.col.name') }}</TableHead>
          <TableHead :class="TH">{{ t('automations.journal.col.status') }}</TableHead>
          <TableHead :class="TH">{{ t('automations.journal.col.did') }}</TableHead>
          <TableHead :class="TH"><span class="sr-only">{{ t('automations.journal.col.actions') }}</span></TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        <TableRow v-for="row in rows" :key="row.entry.id" :data-journal-row="row.entry.status">
          <TableCell class="w-[130px] px-3 py-2.5 text-sm whitespace-nowrap text-muted-foreground">{{ row.when }}</TableCell>
          <TableCell class="max-w-0 w-[37%] px-3 py-2.5 font-medium">
            <RouterLink
              v-if="opens(row.entry)"
              :to="entityLocation({ kind: 'automation', id: row.entry.source_id }, route.query)"
              class="block truncate rounded-sm hover:underline focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
            >
              {{ row.entry.name }}
            </RouterLink>
            <span v-else class="block truncate" :title="t('automations.journal.removed')">{{ row.entry.name }}</span>
          </TableCell>
          <TableCell class="px-3 py-2.5 whitespace-nowrap" :class="row.tone">{{ row.status }}</TableCell>
          <TableCell class="px-3 py-2.5 text-sm whitespace-nowrap">
            <MoneyPhrase :parts="row.did" />
          </TableCell>
          <TableCell class="px-3 py-1.5 text-right">
            <Button v-if="row.entry.wait_id" size="sm" data-journal-confirm data-tour="journal-confirm" @click="emit('confirm', row.entry)">
              {{ t('automations.journal.confirm') }}
            </Button>
          </TableCell>
        </TableRow>
      </TableBody>
    </Table>
  </div>
</template>
