<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { RouterLink } from 'vue-router'
import { AlarmClock, ExternalLink, Pencil, Repeat, Trash2 } from 'lucide-vue-next'

import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { Checkbox } from '@/shared/ui/checkbox'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { DropdownMenu } from '@/shared/ui/dropdown-menu'
import { DropdownMenuItem } from '@/shared/ui/dropdown-menu-item'
import { formatDateTime } from '@/shared/lib/format'

import { SNOOZES, type SnoozeChoice, dueOf, overdue, repeatLabel, sourceLink } from '../lib/reminder'

/** One reminder: tick it off, put it off (always in sight), and edit or delete on hover. */
const props = defineProps<{ reminder: Reminder }>()
const emit = defineEmits<{ done: []; snooze: [choice: SnoozeChoice]; finish: []; edit: []; remove: [] }>()
const { t } = useI18n()
</script>

<template>
  <li
    class="group flex items-center gap-3 rounded-md px-3 py-2 hover:bg-accent/50"
    :class="props.reminder.done && 'opacity-60'"
  >
    <Checkbox
      :model-value="props.reminder.done"
      :aria-label="t('reminder.done')"
      :title="props.reminder.repeat ? t('reminder.doneRepeatHint') : t('reminder.done')"
      @update:model-value="emit('done')"
    />
    <span class="flex min-w-0 flex-1 items-center gap-2">
      <span class="truncate text-sm" :class="props.reminder.done && 'line-through'">{{ props.reminder.text }}</span>
      <Badge v-if="props.reminder.repeat" variant="outline" class="shrink-0 gap-1">
        <Repeat class="size-3" />
        {{ repeatLabel(props.reminder.repeat) }}
      </Badge>
      <Badge v-if="props.reminder.source" variant="secondary" class="shrink-0" :title="t('reminder.autoHint')">
        {{ t('reminder.auto') }}
      </Badge>
      <RouterLink
        v-if="props.reminder.source && sourceLink(props.reminder.source)"
        :to="sourceLink(props.reminder.source) ?? '/'"
        class="shrink-0 text-muted-foreground hover:text-foreground"
        :title="t('reminder.openSource')"
        :aria-label="t('reminder.openSource')"
      >
        <ExternalLink class="size-3.5" />
      </RouterLink>
    </span>
    <span
      class="shrink-0 text-xs tabular-nums"
      :class="overdue(props.reminder) ? 'text-destructive' : 'text-muted-foreground'"
    >
      <AlarmClock
        v-if="props.reminder.snoozed_until && dueOf(props.reminder) === props.reminder.snoozed_until"
        class="mr-1 inline size-3"
      />
      {{ formatDateTime(dueOf(props.reminder)) }}
    </span>
    <DropdownMenu v-if="!props.reminder.done" align="end">
      <template #trigger>
        <Button variant="ghost" size="sm" class="h-8 gap-1 px-2 text-muted-foreground" :title="t('reminder.snoozeAction')">
          <AlarmClock class="size-4" />
          <span class="max-sm:sr-only">{{ t('reminder.snoozeAction') }}</span>
        </Button>
      </template>
      <DropdownMenuItem v-for="choice in SNOOZES" :key="choice" :value="`snooze-${choice}`" @select="emit('snooze', choice)">
        <AlarmClock class="size-4 shrink-0" />
        {{ t(`reminder.snooze.${choice}`) }}
      </DropdownMenuItem>
      <DropdownMenuItem v-if="props.reminder.repeat || props.reminder.source" value="finish" @select="emit('finish')">
        <Repeat class="size-4 shrink-0" />
        {{ props.reminder.source ? t('reminder.mute') : t('reminder.finishSeries') }}
      </DropdownMenuItem>
    </DropdownMenu>
    <div
      class="flex shrink-0 items-center gap-1 transition-opacity pointer-fine:opacity-0 pointer-fine:group-hover:opacity-100 pointer-fine:group-focus-within:opacity-100"
    >
      <Button
        variant="ghost"
        size="icon"
        class="size-8 text-muted-foreground"
        :title="t('reminder.edit')"
        :aria-label="t('reminder.edit')"
        @click="emit('edit')"
      >
        <Pencil class="size-4" />
      </Button>
      <ConfirmButton :label="t('reminder.delete')" :confirm-label="t('actions.confirmDelete')" @confirm="emit('remove')">
        <Trash2 class="size-4" />
      </ConfirmButton>
    </div>
  </li>
</template>
