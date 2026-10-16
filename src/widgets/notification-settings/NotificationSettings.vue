<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery } from '@tanstack/vue-query'
import { BellRing } from 'lucide-vue-next'

import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'
import { TimeInput } from '@/shared/ui/time-input'
import { Label } from '@/shared/ui/label'
import { Switch } from '@/shared/ui/switch'
import { call } from '@/shared/api/ipc'
import { useSettingsStore } from '@/shared/store/settings'
import { notify } from '@/shared/lib/notify'

import {
  AUTO_REMINDER_CATEGORIES,
  DEFAULT_LEAD_DAYS,
  LEAD_CATEGORIES,
  NOTICE_CATEGORIES,
} from './notification-settings.constants'

/** «Уведомления»: what to be told about, how far ahead, and how. */
const { t } = useI18n()
const settings = useSettingsStore()

const current = computed<NotificationSettings>(
  () =>
    settings.vault?.notifications ?? { system: true, hide_details_when_locked: false, default_time: '10:00', categories: [] },
)

function category(name: NoticeCategory): NoticeCategoryCfg {
  return (
    current.value.categories.find((entry) => entry.category === name) ?? {
      category: name,
      enabled: true,
      lead_days: DEFAULT_LEAD_DAYS[name] ?? 0,
      auto_reminders: true,
    }
  )
}

async function save(patch: Partial<NotificationSettings>) {
  try {
    await settings.updateVault({ notifications: { ...current.value, ...patch } })
    notify.saved()
  } catch (err) {
    notify.error(err, 'settings.saveFailed')
  }
}

function patchCategory(name: NoticeCategory, patch: Partial<NoticeCategoryCfg>) {
  const next = { ...category(name), ...patch }
  const others = current.value.categories.filter((entry) => entry.category !== name)
  void save({ categories: [...others, next] })
}

const leadOf = (raw: string | number) => Math.max(0, Math.min(90, Math.round(Number(raw) || 0)))

const permission = useQuery({
  queryKey: ['notification-permission'],
  queryFn: () => call<string>('notification_permission', { request: false }),
})
const askPermission = useMutation({
  mutationFn: () => call<string>('notification_permission', { request: true }),
  onSuccess: () => void permission.refetch(),
  meta: {
    error: 'settings.saveFailed',
    success: (data) => `${t('notices.permission')}: ${t(`notices.permissionState.${data as string}`)}`,
  },
})
/* --- while the vault is locked, and after a reboot --- */
const storage = useQuery({ queryKey: ['notification-storage'], queryFn: () => call<string>('notification_storage') })
const autostart = useQuery({ queryKey: ['autostart'], queryFn: () => call<boolean>('get_autostart') })
const setAutostart = useMutation({
  mutationFn: (enabled: boolean) => call<boolean>('set_autostart', { enabled }),
  onSuccess: () => void autostart.refetch(),
  meta: {
    error: 'settings.saveFailed',
    success: (data) => (data ? t('notices.autostartOn') : t('notices.autostartOff')),
  },
})

const test = useMutation({
  mutationFn: () => call('send_test_notification'),
  meta: { success: 'notices.testSent', error: 'notices.testFailed' },
})
</script>

<template>
  <div class="grid gap-5" data-tour="notification-settings">
    <div class="grid gap-3 rounded-lg border border-border p-4">
      <div class="flex items-center justify-between gap-3">
        <div class="grid gap-0.5">
          <Label for="notice-system">{{ t('notices.system') }}</Label>
          <p class="text-xs text-muted-foreground">{{ t('notices.systemHint') }}</p>
        </div>
        <Switch id="notice-system" :model-value="current.system" @update:model-value="(value) => save({ system: Boolean(value) })" />
      </div>
      <div class="flex items-center justify-between gap-3">
        <div class="grid gap-0.5">
          <Label for="notice-hide">{{ t('notices.hideDetails') }}</Label>
          <p class="text-xs text-muted-foreground">{{ t('notices.hideDetailsHint') }}</p>
        </div>
        <Switch
          id="notice-hide"
          :model-value="current.hide_details_when_locked"
          @update:model-value="(value) => save({ hide_details_when_locked: Boolean(value) })"
        />
      </div>
      <div class="flex items-center justify-between gap-3">
        <div class="grid gap-0.5">
          <Label for="notice-time">{{ t('notices.defaultTime') }}</Label>
          <p class="text-xs text-muted-foreground">{{ t('notices.defaultTimeHint') }}</p>
        </div>
        <TimeInput
          id="notice-time"
          class="w-24"
          :model-value="current.default_time"
          @update:model-value="(next) => save({ default_time: next || '10:00' })"
        />
      </div>
      <div class="flex flex-wrap items-center justify-between gap-3">
        <div class="flex items-center gap-2 text-sm">
          {{ t('notices.permission') }}
          <Badge :variant="permission.data.value === 'granted' ? 'secondary' : 'destructive'">
            {{ t(`notices.permissionState.${permission.data.value ?? 'prompt'}`) }}
          </Badge>
          <Button
            v-if="permission.data.value && permission.data.value !== 'granted'"
            variant="outline"
            size="sm"
            @click="askPermission.mutate()"
          >
            {{ t('notices.askPermission') }}
          </Button>
        </div>
        <Button variant="outline" size="sm" :disabled="test.isPending.value" @click="test.mutate()">
          <BellRing class="size-4" />
          {{ t('notices.test') }}
        </Button>
      </div>
    </div>

    <div class="grid gap-3 rounded-lg border border-border p-4" data-tour="notification-locked">
      <div class="grid gap-0.5">
        <p class="text-sm font-medium">{{ t('notices.locked') }}</p>
        <p class="text-xs text-muted-foreground">
          {{ storage.data.value === 'keyring' ? t('notices.lockedKeyring') : t('notices.lockedMemory') }}
        </p>
      </div>
      <div class="flex items-center justify-between gap-3">
        <div class="grid gap-0.5">
          <Label for="notice-autostart">{{ t('notices.autostart') }}</Label>
          <p class="text-xs text-muted-foreground">{{ t('notices.autostartHint') }}</p>
        </div>
        <Switch
          id="notice-autostart"
          :model-value="autostart.data.value ?? false"
          :disabled="setAutostart.isPending.value"
          @update:model-value="(value) => setAutostart.mutate(Boolean(value))"
        />
      </div>
    </div>

    <div class="grid gap-1 rounded-lg border border-border">
      <div class="grid grid-cols-[minmax(0,1fr)_4.5rem_6rem_6rem] items-center gap-2 border-b border-border px-4 py-2 text-xs text-muted-foreground">
        <span>{{ t('notices.category') }}</span>
        <span class="text-center">{{ t('notices.enabled') }}</span>
        <span class="text-center">{{ t('notices.leadDays') }}</span>
        <span class="text-center">{{ t('notices.autoReminders') }}</span>
      </div>
      <div
        v-for="name in NOTICE_CATEGORIES"
        :key="name"
        class="grid grid-cols-[minmax(0,1fr)_4.5rem_6rem_6rem] items-center gap-2 px-4 py-1.5"
      >
        <span class="grid min-w-0">
          <span class="truncate text-sm">{{ t(`notices.cat.${name}`) }}</span>
          <span class="truncate text-xs text-muted-foreground">{{ t(`notices.catHint.${name}`) }}</span>
        </span>
        <span class="flex justify-center">
          <Switch
            :model-value="category(name).enabled"
            :aria-label="t('notices.enabledFor', { name: t(`notices.cat.${name}`) })"
            @update:model-value="(value) => patchCategory(name, { enabled: Boolean(value) })"
          />
        </span>
        <span class="flex justify-center">
          <Input
            v-if="LEAD_CATEGORIES.includes(name)"
            type="number"
            min="0"
            max="90"
            class="h-8 w-16 px-2 text-center"
            :model-value="String(category(name).lead_days)"
            :aria-label="t('notices.leadDaysFor', { name: t(`notices.cat.${name}`) })"
            @change="(event: Event) => patchCategory(name, { lead_days: leadOf((event.target as HTMLInputElement).value) })"
          />
        </span>
        <span class="flex justify-center">
          <Switch
            v-if="AUTO_REMINDER_CATEGORIES.includes(name)"
            :model-value="category(name).auto_reminders"
            :aria-label="t('notices.autoRemindersFor', { name: t(`notices.cat.${name}`) })"
            @update:model-value="(value) => patchCategory(name, { auto_reminders: Boolean(value) })"
          />
        </span>
      </div>
    </div>
  </div>
</template>
