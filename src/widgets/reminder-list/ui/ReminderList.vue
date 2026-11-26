<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { keepPreviousData, useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { Pencil, Plus, Repeat, Trash2 } from 'lucide-vue-next'

import { ReminderDialog } from '@/features/reminder-edit'
import { Button } from '@/shared/ui/button'
import { call } from '@/shared/api/ipc'
import { formatDateTime } from '@/shared/lib/format'
import { PLANNING_KEYS } from '@/shared/lib/planning'
import { Badge } from '@/shared/ui/badge'
import { Card } from '@/shared/ui/card'
import { Checkbox } from '@/shared/ui/checkbox'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { SectionState } from '@/shared/ui/section-state'

import { dueOf, groupReminders } from '../lib/reminder'

/** Reminders under their headings: overdue, today, later, done. «Done» is a checkbox; delete asks first. */
const { t } = useI18n()
const queryClient = useQueryClient()
const reminders = useQuery({
  queryKey: ['reminders'],
  queryFn: () => call<Reminder[]>('list_reminders'),
  placeholderData: keepPreviousData,
})
const groups = computed(() => groupReminders(reminders.data.value ?? []))
const refresh = () => {
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
const remove = useMutation({
  mutationFn: (id: string) => call('delete_reminder', { id }),
  onSuccess: refresh,
  meta: { error: 'reminder.deleteFailed', success: 'reminder.deleted' },
})

const dialogOpen = ref(false)
const editing = ref<Reminder | null>(null)
function openDialog(reminder: Reminder | null) {
  editing.value = reminder
  dialogOpen.value = true
}
</script>

<template>
  <div class="grid gap-3">
    <div class="flex flex-wrap items-center justify-between gap-2">
      <p class="text-sm text-muted-foreground">{{ t('reminder.intro') }}</p>
      <Button size="sm" data-tour="reminder-new" @click="openDialog(null)">
        <Plus class="size-4" />
        {{ t('reminder.new') }}
      </Button>
    </div>
    <SectionState
      :loading="reminders.isPending.value"
      :error="reminders.error.value"
      :empty="!groups.length"
      :empty-title="t('reminder.empty')"
      :rows="4"
      @retry="reminders.refetch()"
    >
      <section v-for="group in groups" :key="group.key" class="grid gap-1.5" :data-reminder-group="group.key">
        <h3 class="text-xs font-medium text-muted-foreground">{{ t(`reminder.group.${group.key}`) }}</h3>
        <Card size="sm">
          <ul class="divide-y divide-border">
            <li v-for="reminder in group.items" :key="reminder.id" class="flex min-h-14 items-center gap-3 py-2">
              <Checkbox
                :model-value="reminder.done"
                :aria-label="t('reminder.done')"
                @update:model-value="setDone.mutate(reminder)"
              />
              <div class="min-w-0 flex-1">
                <p class="truncate text-sm font-medium" :class="reminder.done && 'text-muted-foreground line-through'">
                  {{ reminder.text }}
                </p>
                <p class="flex items-center gap-1 truncate text-xs text-muted-foreground">
                  <Repeat v-if="reminder.repeat" class="size-3" />
                  {{ formatDateTime(dueOf(reminder)) }}
                </p>
              </div>
              <Badge v-if="reminder.source" variant="secondary">{{ t('reminder.auto') }}</Badge>
              <Button variant="ghost" size="icon-sm" :aria-label="t('reminder.edit')" @click="openDialog(reminder)">
                <Pencil class="size-4" />
              </Button>
              <ConfirmButton
                :label="t('reminder.delete')"
                :confirm-label="t('actions.confirmDelete')"
                @confirm="remove.mutate(reminder.id)"
              >
                <Trash2 class="size-4" />
              </ConfirmButton>
            </li>
          </ul>
        </Card>
      </section>
    </SectionState>
    <ReminderDialog v-model:open="dialogOpen" :reminder="editing" />
  </div>
</template>
