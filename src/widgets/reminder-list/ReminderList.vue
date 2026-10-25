<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { Plus } from 'lucide-vue-next'

import { ReminderDialog } from '@/features/reminder-edit'
import { Button } from '@/shared/ui/button'
import { ListHeader } from '@/shared/ui/list-header'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'
import { EmptyState } from '@/shared/ui/empty-state'
import { call } from '@/shared/api/ipc'
import { formatDateTime } from '@/shared/lib/format'
import { useFocusRow } from '@/shared/lib/use-focus-row'
import { PLANNING_KEYS } from '@/shared/lib/planning'

import { type ReminderGroup, type SnoozeChoice, groupOf, snoozeUntil } from './lib/reminder'
import ReminderRow from './ui/ReminderRow.vue'

const { t } = useI18n()
const queryClient = useQueryClient()

const reminders = useQuery({ queryKey: ['reminders'], queryFn: () => call<Reminder[]>('list_reminders') })
useFocusRow(() => reminders.isSuccess.value)
/** «Мои» are written by the user, «Авто» are made from the data. */
const show = ref<'all' | 'mine' | 'auto'>('all')
/** By time; grouped below into overdue, today, later and done. Muted automatic ones are hidden. */
const list = computed(() =>
  [...(reminders.data.value ?? [])]
    .filter((reminder) => !(reminder.source && reminder.muted))
    .filter((reminder) => show.value === 'all' || (show.value === 'auto') === Boolean(reminder.source))
    .sort(
    (left, right) => Number(left.done) - Number(right.done) || left.due_at.localeCompare(right.due_at),
  ),
)
const GROUPS: ReminderGroup[] = ['overdue', 'today', 'later', 'done']
const groups = computed(() =>
  GROUPS.map((key) => ({ key, items: list.value.filter((reminder) => groupOf(reminder) === key) })).filter(
    (group) => group.items.length,
  ),
)


function refresh() {
  for (const queryKey of PLANNING_KEYS) void queryClient.invalidateQueries({ queryKey })
}

const setDone = useMutation({
  mutationFn: (reminder: Reminder) => call<Reminder>('set_reminder_done', { id: reminder.id, done: !reminder.done }),
  onSuccess: refresh,
  meta: {
    error: 'reminder.saveFailed',
    success: (_data, reminder) => ((reminder as Reminder).done ? t('reminder.reopened') : t('reminder.done')),
  },
})

const snooze = useMutation({
  mutationFn: ({ reminder, choice }: { reminder: Reminder; choice: SnoozeChoice }) =>
    call<Reminder>('snooze_reminder', { input: { id: reminder.id, until: snoozeUntil(choice) } }),
  onSuccess: refresh,
  meta: {
    error: 'reminder.saveFailed',
    success: (data) => t('reminder.snoozed', { when: formatDateTime((data as Reminder).snoozed_until) }),
  },
})
const finish = useMutation({
  mutationFn: (reminder: Reminder) => call<Reminder>('finish_reminder_series', { id: reminder.id }),
  onSuccess: () => {
    refresh()
  },
  meta: { error: 'reminder.saveFailed', success: 'reminder.seriesFinished' },
})

const remove = useMutation({
  mutationFn: (id: string) => call('delete_reminder', { id }),
  onSuccess: () => {
    refresh()
  },
  meta: { error: 'reminder.deleteFailed', success: 'reminder.deleted' },
})

const dialogOpen = ref(false)
const editing = ref<Reminder | null>(null)
function openNew() {
  editing.value = null
  dialogOpen.value = true
}
function openEdit(reminder: Reminder) {
  editing.value = reminder
  dialogOpen.value = true
}
</script>

<template>
  <div class="grid grid-cols-1 gap-4">
    <ListHeader
      :intro="t('reminder.intro')"
      :action-label="t('reminder.new')"
      :hide-action="reminders.isSuccess.value && !list.length"
      @action="openNew"
    />

    <ToggleGroup
      v-model="show"
      type="single"
      variant="outline"
      size="sm"
      class="w-fit"
      :aria-label="t('reminder.filter')"
      data-tour="reminder-filter"
    >
      <ToggleGroupItem value="all">{{ t('reminder.filterAll') }}</ToggleGroupItem>
      <ToggleGroupItem value="mine">{{ t('reminder.filterMine') }}</ToggleGroupItem>
      <ToggleGroupItem value="auto">{{ t('reminder.filterAuto') }}</ToggleGroupItem>
    </ToggleGroup>

    <EmptyState v-if="reminders.isSuccess.value && !list.length" compact :title="t('reminder.empty')">
      <Button @click="openNew"><Plus class="size-4" />{{ t('reminder.new') }}</Button>
    </EmptyState>

    <section v-for="group in groups" :key="group.key" class="grid grid-cols-1 gap-1">
      <h3
        class="px-3 text-xs font-medium uppercase tracking-wide"
        :class="group.key === 'overdue' ? 'text-destructive' : 'text-muted-foreground'"
      >
        {{ t(`reminder.group.${group.key}`) }}
      </h3>
      <ul class="grid grid-cols-1 gap-1">
        <ReminderRow
          v-for="reminder in group.items"
          :key="reminder.id"
          :data-focus-row="reminder.id"
          :reminder="reminder"
          @done="setDone.mutate(reminder)"
          @snooze="(choice) => snooze.mutate({ reminder, choice })"
          @finish="finish.mutate(reminder)"
          @edit="openEdit(reminder)"
          @remove="remove.mutate(reminder.id)"
        />
      </ul>
    </section>

    <ReminderDialog v-model:open="dialogOpen" :reminder="editing" />
  </div>
</template>
