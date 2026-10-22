<script setup lang="ts">
import { type RouteLocationRaw, RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { formatDate, formatQuantity } from '@/shared/lib/format'

/** Up to three things to do today, each a link to where it is done. */
const props = defineProps<{ todos: TodayTodo[]; currency: string }>()
const { t } = useI18n()

const WHERE: Record<TodayTodoKind, (todo: TodayTodo) => RouteLocationRaw> = {
  shortfall: () => ({ name: 'transactions-calendar' }),
  rejected_run: () => ({ name: 'workflows-journal' }),
  unconfirmed_run: () => ({ name: 'workflows-journal' }),
  queued_workflow: () => ({ name: 'workflows-journal' }),
  pending_import: (todo) => ({ name: 'wallet', params: { id: todo.source_id } }),
  points_expire: () => ({ name: 'transactions-bonuses' }),
  insurance_ends: (todo) => ({ name: 'wallet', params: { id: todo.source_id } }),
}

/** Money for a shortfall or a run; a count or points otherwise. */
const isMoney = (kind: TodayTodoKind) => kind === 'shortfall' || kind === 'rejected_run' || kind === 'unconfirmed_run'
</script>

<template>
  <ul class="grid gap-1 text-sm">
    <li v-for="todo in props.todos" :key="`${todo.kind}:${todo.source_id}:${todo.date}`" class="min-w-0">
      <RouterLink
        :to="WHERE[todo.kind](todo)"
        class="flex min-w-0 items-baseline gap-2 rounded-sm hover:underline focus-visible:outline-2 focus-visible:outline-ring"
      >
        <span class="shrink-0 text-xs text-muted-foreground tabular-nums">
          {{ todo.date ? formatDate(todo.date) : t('dashboard.today.now') }}
        </span>
        <span class="min-w-0 truncate">{{ t(`dashboard.today.todo.${todo.kind}`, { name: todo.title }) }}</span>
        <span v-if="todo.amount !== null" class="ml-auto shrink-0 text-xs tabular-nums">
          <Money v-if="isMoney(todo.kind)" :amount="todo.amount" :currency="props.currency" :decimals="0" :convert="false" />
          <template v-else>{{ formatQuantity(todo.amount, 0) }}</template>
        </span>
      </RouterLink>
    </li>
  </ul>
</template>
