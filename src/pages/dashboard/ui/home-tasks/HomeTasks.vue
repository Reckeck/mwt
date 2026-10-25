<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useLocalStorage } from '@vueuse/core'
import { Bell } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { formatDayMonth, formatQuantity } from '@/shared/lib/format'
import { dayKey } from '@/shared/lib/period'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'

import { isMoneyTask, isOverdue, taskAction } from '../../lib/task-action'
import TaskButton from './TaskButton.vue'

/**
 * «Дела» on Home (Home 1, 2): up to three things, each with the one button that
 * does it. One thing is a single line. «Скрыть до завтра» is this device's choice.
 */
const props = defineProps<{ todos: TodayTodo[]; total: number; currency: string }>()
const { t } = useI18n()

const today = dayKey()
const hiddenOn = useLocalStorage('trade-app:home-tasks-hidden', '')
const hidden = computed(() => hiddenOn.value === today)

/** The line under a thing's title: words, and a sum drawn as `<Money>`. */
type MetaPart = { text: string } | { amount: string }
function meta(todo: TodayTodo): MetaPart[] {
  const parts: MetaPart[] = []
  const word = (text: string) => parts.push({ text })
  if (todo.kind === 'reminder') word(t('home.tasks.reminder'))
  if (todo.date && isOverdue(todo, today)) word(t('home.tasks.wasDue', { date: formatDayMonth(`${todo.date}T12:00`) }))
  else if (todo.date && todo.date > today) word(formatDayMonth(`${todo.date}T12:00`))
  if (todo.amount !== null) {
    if (isMoneyTask(todo.kind)) parts.push({ amount: todo.amount })
    else word(formatQuantity(todo.amount, 0))
  }
  return parts
}
const rows = computed(() =>
  props.todos.map((todo) => {
    const action = taskAction(todo)
    return {
      key: `${todo.kind}:${todo.source_id}:${todo.date}`,
      title: t(`home.tasks.todo.${todo.kind}`, { name: todo.title }),
      meta: meta(todo),
      overdue: isOverdue(todo, today),
      to: action.to,
      label: t(`home.tasks.action.${action.label}`),
    }
  }),
)
const overdue = computed(() => rows.value.filter((row) => row.overdue).length)
</script>

<template>
  <Card v-if="rows.length && !hidden" data-tour="home-tasks">
    <!-- One thing: a single line, the button at its end. -->
    <div v-if="props.total === 1" class="flex min-w-0 items-center gap-3">
      <Bell class="size-4 shrink-0" aria-hidden="true" />
      <span class="shrink-0 font-semibold max-md:sr-only">{{ t('home.tasks.title', { count: props.total }) }}</span>
      <span class="min-w-0 flex-1 truncate text-sm">{{ rows[0].title }}</span>
      <TaskButton :to="rows[0].to" :label="rows[0].label" />
    </div>

    <div v-else class="-mb-1 grid">
      <div class="flex min-w-0 items-center gap-2 pb-1.5">
        <span class="text-base font-semibold">{{ t('home.tasks.title', { count: props.total }) }}</span>
        <span v-if="overdue" class="rounded-md bg-warning/15 px-2 py-0.5 text-xs font-medium text-warning">
          {{ t('home.tasks.overdue', { count: overdue }) }}
        </span>
        <Button variant="ghost" size="sm" class="-my-1 ml-auto text-muted-foreground max-md:h-10" @click="hiddenOn = today">
          {{ t('home.tasks.hide') }}
        </Button>
      </div>
      <div v-for="row in rows" :key="row.key" class="flex min-w-0 items-center gap-3 border-t border-border py-2">
        <span v-if="row.overdue" class="size-2 shrink-0 rounded-full bg-warning" aria-hidden="true" />
        <p class="min-w-0 flex-1 text-sm font-medium">
          {{ row.title }}
          <span v-if="row.meta.length" class="font-normal text-muted-foreground">
            <template v-for="(part, at) in row.meta" :key="at">
              {{ ' · ' }}<Money v-if="'amount' in part" :amount="part.amount" :currency="props.currency" />
              <template v-else>{{ part.text }}</template>
            </template>
          </span>
        </p>
        <TaskButton :to="row.to" :label="row.label" />
      </div>
      <p v-if="props.total > rows.length" class="border-t border-border py-2 text-xs text-muted-foreground">
        {{ t('home.tasks.more', { count: props.total - rows.length }) }}
      </p>
    </div>
  </Card>
</template>
