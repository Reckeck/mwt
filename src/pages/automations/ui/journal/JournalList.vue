<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { MoneyPhrase } from '@/entities/currency'
import { Button } from '@/shared/ui/button'

import type { JournalRow } from './journal.types'

/**
 * The journal on a phone (Auto-1 · 4): the name, `when · what it did` under
 * it, the outcome on the right. A run waiting for `Yes` has its own `Confirm`.
 */
defineProps<{ rows: JournalRow[] }>()
const emit = defineEmits<{ confirm: [entry: JournalEntry] }>()
const { t } = useI18n()
</script>

<template>
  <ul class="rounded-xl border border-border bg-card px-3 shadow-sm" data-journal-list>
    <li
      v-for="row in rows"
      :key="row.entry.id"
      class="relative flex min-h-13 items-center gap-3 border-b border-border py-1.5 last:border-b-0"
      :data-journal-row="row.entry.status"
    >
      <div class="flex min-w-0 flex-1 flex-col">
        <span class="truncate font-medium">{{ row.entry.name }}</span>
        <span class="truncate text-xs text-muted-foreground">
          {{ row.when }} ·
          <MoneyPhrase :parts="row.did" />
        </span>
      </div>
      <div v-if="row.entry.wait_id" class="flex shrink-0 items-center gap-2">
        <span class="sr-only">{{ row.status }}</span>
        <Button
          size="sm"
          class="after:absolute after:inset-0"
          :aria-label="t('automations.journal.confirmName', { name: row.entry.name })"
          data-journal-confirm
          data-tour="journal-confirm"
          @click="emit('confirm', row.entry)"
        >
          {{ t('automations.journal.confirm') }}
        </Button>
      </div>
      <span v-else class="shrink-0 text-sm" :class="row.tone">{{ row.status }}</span>
    </li>
  </ul>
</template>
