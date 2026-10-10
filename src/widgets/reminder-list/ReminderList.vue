<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { Pencil, Plus, Trash2 } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { ReminderDialog } from '@/features/reminder-edit'
import { Button } from '@/shared/ui/button'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { call, parseError } from '@/shared/api/ipc'
import { formatDateTime } from '@/shared/lib/format'
import { PLANNING_KEYS } from '@/shared/lib/planning'
import { codeErrorMessage } from '@/shared/lib/vault-error'

const { t } = useI18n()
const queryClient = useQueryClient()

const reminders = useQuery({ queryKey: ['reminders'], queryFn: () => call<Reminder[]>('list_reminders') })
/** Open ones first, by time; done ones after. */
const list = computed(() =>
  [...(reminders.data.value ?? [])].sort(
    (left, right) => Number(left.done) - Number(right.done) || left.due_at.localeCompare(right.due_at),
  ),
)
const overdue = (reminder: Reminder) => !reminder.done && new Date(reminder.due_at).getTime() < Date.now()

function refresh() {
  for (const queryKey of PLANNING_KEYS) void queryClient.invalidateQueries({ queryKey })
}

const setDone = useMutation({
  mutationFn: (reminder: Reminder) => call<Reminder>('set_reminder_done', { id: reminder.id, done: !reminder.done }),
  onSuccess: refresh,
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
          @change="setDone.mutate(reminder)"
        />
        <span class="min-w-0 flex-1 truncate text-sm" :class="reminder.done && 'line-through'">{{ reminder.text }}</span>
        <span class="shrink-0 text-xs tabular-nums" :class="overdue(reminder) ? 'text-destructive' : 'text-muted-foreground'">
          {{ formatDateTime(reminder.due_at) }}
        </span>
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
