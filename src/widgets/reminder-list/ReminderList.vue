<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { AlarmClock, ExternalLink, MoreVertical, Pencil, Plus, Repeat, Trash2 } from 'lucide-vue-next'
import { RouterLink, type RouteLocationRaw } from 'vue-router'
import { toast } from 'vue-sonner'

import { ReminderDialog } from '@/features/reminder-edit'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { DropdownMenu } from '@/shared/ui/dropdown-menu'
import { DropdownMenuItem } from '@/shared/ui/dropdown-menu-item'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'
import { onEvent } from '@/shared/api/ipc'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { call, parseError } from '@/shared/api/ipc'
import { formatDateTime } from '@/shared/lib/format'
import { PLANNING_KEYS } from '@/shared/lib/planning'
import { codeErrorMessage } from '@/shared/lib/vault-error'

const { t } = useI18n()
const queryClient = useQueryClient()

const reminders = useQuery({ queryKey: ['reminders'], queryFn: () => call<Reminder[]>('list_reminders') })
/** «Мои» are written by the user, «Авто» are made from the data. */
const show = ref<'all' | 'mine' | 'auto'>('all')
/** Open ones first, by time; done ones after. Muted automatic ones are hidden. */
const list = computed(() =>
  [...(reminders.data.value ?? [])]
    .filter((reminder) => !(reminder.source && reminder.muted))
    .filter((reminder) => show.value === 'all' || (show.value === 'auto') === Boolean(reminder.source))
    .sort(
    (left, right) => Number(left.done) - Number(right.done) || left.due_at.localeCompare(right.due_at),
  ),
)
/** When it comes next: its date, or the end of its snooze. */
const dueOf = (reminder: Reminder) =>
  reminder.snoozed_until && reminder.snoozed_until > reminder.due_at ? reminder.snoozed_until : reminder.due_at
const overdue = (reminder: Reminder) => !reminder.done && new Date(dueOf(reminder)).getTime() < Date.now()
const repeatLabel = (repeat: ReminderRepeat) =>
  `${t('reminder.every')} ${repeat.every > 1 ? `${repeat.every} ` : ''}${t(`reminder.unit.${repeat.unit}`, repeat.every)}`

// The scheduler made or moved automatic reminders.
void onEvent('reminders-changed', () => void queryClient.invalidateQueries({ queryKey: ['reminders'] }))

/** Where an automatic reminder comes from: the wallet, the goal, the contact. */
function sourceLink(source: ReminderSource): RouteLocationRaw | null {
  switch (source.kind) {
    case 'goal_saving':
    case 'goal_deadline':
      return { name: 'planning-goals' }
    case 'debt':
      return { name: 'directory-contacts' }
    case 'rent':
    case 'property_tax':
    case 'interest_tax':
      return null
    default:
      return { name: 'wallet', params: { id: source.id } }
  }
}

function refresh() {
  for (const queryKey of PLANNING_KEYS) void queryClient.invalidateQueries({ queryKey })
}

const setDone = useMutation({
  mutationFn: (reminder: Reminder) => call<Reminder>('set_reminder_done', { id: reminder.id, done: !reminder.done }),
  onSuccess: refresh,
  onError: (err) => toast.error(t('reminder.saveFailed'), { description: codeErrorMessage(parseError(err), t) }),
})

/* --- put off: in ten minutes, in an hour, tomorrow morning --- */
const SNOOZES = ['10m', '1h', 'tomorrow'] as const
function snoozeUntil(choice: (typeof SNOOZES)[number]): string {
  const at = new Date()
  if (choice === '10m') at.setMinutes(at.getMinutes() + 10)
  else if (choice === '1h') at.setHours(at.getHours() + 1)
  else {
    at.setDate(at.getDate() + 1)
    at.setHours(9, 0, 0, 0)
  }
  return at.toISOString()
}
const snooze = useMutation({
  mutationFn: ({ reminder, choice }: { reminder: Reminder; choice: (typeof SNOOZES)[number] }) =>
    call<Reminder>('snooze_reminder', { input: { id: reminder.id, until: snoozeUntil(choice) } }),
  onSuccess: (reminder) => {
    toast.success(t('reminder.snoozed', { when: formatDateTime(reminder.snoozed_until) }))
    refresh()
  },
  onError: (err) => toast.error(t('reminder.saveFailed'), { description: codeErrorMessage(parseError(err), t) }),
})
const finish = useMutation({
  mutationFn: (reminder: Reminder) => call<Reminder>('finish_reminder_series', { id: reminder.id }),
  onSuccess: () => {
    toast.success(t('reminder.seriesFinished'))
    refresh()
  },
  onError: (err) => toast.error(t('reminder.saveFailed'), { description: codeErrorMessage(parseError(err), t) }),
})

const remove = useMutation({
  mutationFn: (id: string) => call('delete_reminder', { id }),
  onSuccess: () => {
    toast.success(t('reminder.deleted'))
    refresh()
  },
  onError: (err) => toast.error(t('reminder.deleteFailed'), { description: codeErrorMessage(parseError(err), t) }),
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
  <div class="grid gap-4">
    <div class="flex flex-wrap items-center justify-between gap-2">
      <p class="text-sm text-muted-foreground">{{ t('reminder.intro') }}</p>
      <Button @click="openNew">
        <Plus class="size-4" />
        {{ t('reminder.new') }}
      </Button>
    </div>

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

    <div
      v-if="reminders.isSuccess.value && !list.length"
      class="rounded-xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground"
    >
      {{ t('reminder.empty') }}
    </div>

    <ul class="grid gap-1">
      <li
        v-for="reminder in list"
        :key="reminder.id"
        class="flex items-center gap-3 rounded-md px-3 py-2 hover:bg-accent/50"
        :class="reminder.done && 'opacity-60'"
      >
        <input
          type="checkbox"
          class="size-4 shrink-0 accent-primary"
          :checked="reminder.done"
          :aria-label="t('reminder.done')"
          :title="reminder.repeat ? t('reminder.doneRepeatHint') : t('reminder.done')"
          @change="setDone.mutate(reminder)"
        />
        <span class="flex min-w-0 flex-1 items-center gap-2">
          <span class="truncate text-sm" :class="reminder.done && 'line-through'">{{ reminder.text }}</span>
          <Badge v-if="reminder.repeat" variant="outline" class="shrink-0 gap-1">
            <Repeat class="size-3" />
            {{ repeatLabel(reminder.repeat) }}
          </Badge>
          <Badge v-if="reminder.source" variant="secondary" class="shrink-0" :title="t('reminder.autoHint')">
            {{ t('reminder.auto') }}
          </Badge>
          <RouterLink
            v-if="reminder.source && sourceLink(reminder.source)"
            :to="sourceLink(reminder.source) ?? '/'"
            class="shrink-0 text-muted-foreground hover:text-foreground"
            :title="t('reminder.openSource')"
            :aria-label="t('reminder.openSource')"
          >
            <ExternalLink class="size-3.5" />
          </RouterLink>
        </span>
        <span class="shrink-0 text-xs tabular-nums" :class="overdue(reminder) ? 'text-destructive' : 'text-muted-foreground'">
          <AlarmClock v-if="reminder.snoozed_until && dueOf(reminder) === reminder.snoozed_until" class="mr-1 inline size-3" />
          {{ formatDateTime(dueOf(reminder)) }}
        </span>
        <DropdownMenu v-if="!reminder.done" align="end">
          <template #trigger>
            <Button
              variant="ghost"
              size="icon"
              class="size-8 text-muted-foreground"
              :title="t('reminder.more')"
              :aria-label="t('reminder.more')"
            >
              <MoreVertical class="size-4" />
            </Button>
          </template>
          <DropdownMenuItem
            v-for="choice in SNOOZES"
            :key="choice"
            :value="`snooze-${choice}`"
            @select="snooze.mutate({ reminder, choice })"
          >
            <AlarmClock class="size-4 shrink-0" />
            {{ t(`reminder.snooze.${choice}`) }}
          </DropdownMenuItem>
          <DropdownMenuItem v-if="reminder.repeat || reminder.source" value="finish" @select="finish.mutate(reminder)">
            <Repeat class="size-4 shrink-0" />
            {{ reminder.source ? t('reminder.mute') : t('reminder.finishSeries') }}
          </DropdownMenuItem>
        </DropdownMenu>
        <Button
          variant="ghost"
          size="icon"
          class="size-8 text-muted-foreground"
          :title="t('reminder.edit')"
          :aria-label="t('reminder.edit')"
          @click="openEdit(reminder)"
        >
          <Pencil class="size-4" />
        </Button>
        <ConfirmButton :label="t('reminder.delete')" :confirm-label="t('actions.confirmDelete')" @confirm="remove.mutate(reminder.id)">
          <Trash2 class="size-4" />
        </ConfirmButton>
      </li>
    </ul>

    <ReminderDialog v-model:open="dialogOpen" :reminder="editing" />
  </div>
</template>
