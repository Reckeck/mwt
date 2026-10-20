<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { ArrowDownLeft, ArrowUpRight, CalendarClock, Undo2 } from 'lucide-vue-next'
import { Money } from '@/entities/currency'

import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'
import { call } from '@/shared/api/ipc'
import { formatDate, } from '@/shared/lib/format'
import { localDate } from '@/shared/lib/planning'
import { DateTimePicker } from '@/shared/ui/date-time-picker'

import type { DebtPanelProps } from './debt-manage.types'

const props = defineProps<DebtPanelProps>()
/** The page opens the transaction form for the chosen debt operation. */
const emit = defineEmits<{ operation: [effect: DebtEffect] }>()

const { t } = useI18n()
const queryClient = useQueryClient()

const terms = useQuery({
  // no placeholder: another contact's terms must not show under this one
  queryKey: computed(() => ['debt-terms', props.contact.id]),
  queryFn: () => call<DebtTerms[]>('list_debt_terms', { contactId: props.contact.id }),
})

const rows = computed(() =>
  props.balances
    .map((row) => {
      const net = Number(row.receivable) - Number(row.payable)
      const own = terms.data.value?.find((entry) => entry.asset === row.asset.symbol.toUpperCase()) ?? null
      const overdue = Boolean(own?.due_at && net !== 0 && own.due_at < localDate(new Date()))
      return { row, net, terms: own, overdue }
    })
    .filter((entry) => entry.net !== 0),
)

const setTerms = useMutation({
  mutationFn: (draft: DebtTermsDraft) => call<DebtTerms | null>('set_debt_terms', { input: draft }),
  onSuccess: () => {
    void queryClient.invalidateQueries({ queryKey: ['debt-terms'] })
    void queryClient.invalidateQueries({ queryKey: ['upcoming'] })
  },
  meta: { success: 'debt.termsSaved', error: 'debt.termsFailed' },
})

function saveTerms(asset: string, current: DebtTerms | null, patch: { due_at?: string; note?: string }) {
  setTerms.mutate({
    contact_id: props.contact.id,
    asset,
    due_at: patch.due_at ?? current?.due_at ?? null,
    note: patch.note ?? current?.note ?? '',
  })
}

const ACTIONS: { effect: DebtEffect; icon: typeof ArrowUpRight }[] = [
  { effect: 'lend', icon: ArrowUpRight },
  { effect: 'borrow', icon: ArrowDownLeft },
  { effect: 'repay', icon: Undo2 },
  { effect: 'collect', icon: Undo2 },
]
</script>

<template>
  <div class="grid gap-4" data-tour="contact-debts">
    <div class="flex flex-wrap gap-2">
      <Button
        v-for="action in ACTIONS"
        :key="action.effect"
        size="sm"
        variant="outline"
        :disabled="props.contact.archived"
        @click="emit('operation', action.effect)"
      >
        <component :is="action.icon" class="size-4" />
        {{ t(`debt.action.${action.effect}`) }}
      </Button>
    </div>

    <p v-if="!rows.length" class="text-sm text-muted-foreground">{{ t('debt.none') }}</p>

    <ul v-else class="grid gap-2">
      <li v-for="entry in rows" :key="entry.row.asset.symbol" class="grid gap-2 rounded-lg border border-border p-3">
        <div class="flex flex-wrap items-baseline justify-between gap-2">
          <span
            class="text-base font-semibold tabular-nums"
            :class="entry.net > 0 ? 'text-success' : 'text-destructive'"
          >
            {{ entry.net > 0 ? t('debt.owesMe') : t('debt.iOwe') }}:
            <Money :amount="Math.abs(entry.net)" :currency="entry.row.asset.symbol" />
          </span>
          <span v-if="entry.terms?.due_at" class="flex items-center gap-1 text-xs" :class="entry.overdue ? 'text-destructive' : 'text-muted-foreground'">
            <CalendarClock class="size-3.5" />
            {{ entry.overdue ? t('debt.overdue', { date: formatDate(entry.terms.due_at) }) : t('debt.dueOn', { date: formatDate(entry.terms.due_at) }) }}
          </span>
        </div>
        <div class="grid gap-2 @mid:grid-cols-[10rem_1fr]">
          <DateTimePicker
            class="h-8"
            :model-value="entry.terms?.due_at ?? ''"
            :aria-label="t('debt.dueDate')"
            @update:model-value="(value) => saveTerms(entry.row.asset.symbol, entry.terms, { due_at: value ?? '' })"
          />
          <Input
            class="h-8"
            :model-value="entry.terms?.note ?? ''"
            :placeholder="t('debt.notePlaceholder')"
            :aria-label="t('debt.note')"
            @change="(event: Event) => saveTerms(entry.row.asset.symbol, entry.terms, { note: (event.target as HTMLInputElement).value })"
          />
        </div>
      </li>
    </ul>
  </div>
</template>
