<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
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
      <!-- Starting with the system is a device setting: «Это устройство». -->
      <RouterLink :to="{ query: { settings: 'device' } }" class="w-fit text-xs text-primary hover:underline">
        {{ t('notices.autostartMoved') }}
      </RouterLink>
    </div>

    <!-- A table from 36rem; narrower, each category is a card-less block: name and switch,
         then its own labelled lines (the fixed columns left ~60px for the name on a phone). -->
    <div class="@container grid gap-1 rounded-lg border border-border">
      <div
        class="hidden items-center gap-2 border-b border-border px-4 py-2 text-xs text-muted-foreground @xl:grid @xl:grid-cols-[minmax(0,1fr)_4.5rem_6rem_6rem]"
      >
        <span>{{ t('notices.category') }}</span>
        <span class="text-center">{{ t('notices.enabled') }}</span>
        <span class="text-center">{{ t('notices.leadDays') }}</span>
        <span class="text-center">{{ t('notices.autoReminders') }}</span>
      </div>
      <div
        v-for="name in NOTICE_CATEGORIES"
        :key="name"
        class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-x-2 gap-y-1 px-4 py-1.5 @max-xl:border-b @max-xl:border-border @max-xl:py-3 @max-xl:last:border-b-0 @xl:grid-cols-[minmax(0,1fr)_4.5rem_6rem_6rem]"
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
        <span
          class="flex items-center justify-between gap-2 @max-xl:col-span-2 @xl:justify-center"
          :class="!LEAD_CATEGORIES.includes(name) && '@max-xl:hidden'"
        >
          <span class="text-xs text-muted-foreground @xl:hidden">{{ t('notices.leadDays') }}</span>
          <Input
            v-if="LEAD_CATEGORIES.includes(name)"
            type="text" inputmode="numeric"
            min="0"
            max="90"
            class="h-8 w-16 px-2 text-center"
            :model-value="String(category(name).lead_days)"
            :aria-label="t('notices.leadDaysFor', { name: t(`notices.cat.${name}`) })"
            @change="(event: Event) => patchCategory(name, { lead_days: leadOf((event.target as HTMLInputElement).value) })"
          />
        </span>
        <span
          class="flex items-center justify-between gap-2 @max-xl:col-span-2 @xl:justify-center"
          :class="!AUTO_REMINDER_CATEGORIES.includes(name) && '@max-xl:hidden'"
        >
          <span class="text-xs text-muted-foreground @xl:hidden">{{ t('notices.autoReminders') }}</span>
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
