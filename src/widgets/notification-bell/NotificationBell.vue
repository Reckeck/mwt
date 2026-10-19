<script setup lang="ts">
import { computed, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { Bell, CheckCheck } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { EmptyState } from '@/shared/ui/empty-state'
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from '@/shared/ui/sheet'
import { call } from '@/shared/api/ipc'
import { formatDate, formatDateTime } from '@/shared/lib/format'
import { noticeRoute } from '@/shared/lib/notice-link'
import { settingsQuery } from '@/shared/lib/settings-link'
import { cn } from '@/shared/lib/utils'

/** The bell: what was told, by day, newest first; unread ones stand out. */
const { t } = useI18n()
const router = useRouter()
const queryClient = useQueryClient()
const open = ref(false)

const notices = useQuery({ queryKey: ['notifications'], queryFn: () => call<NoticeRecord[]>('list_notifications') })
const unread = computed(() => (notices.data.value ?? []).filter((record) => !record.read).length)

/** Grouped by the day it was shown. */
const days = computed(() => {
  const groups = new Map<string, NoticeRecord[]>()
  for (const record of notices.data.value ?? []) {
    const day = record.delivered_at.slice(0, 10)
    groups.set(day, [...(groups.get(day) ?? []), record])
  }
  return [...groups.entries()]
})

const markRead = useMutation({
  mutationFn: (keys: string[] | null) => call('mark_notifications_read', { keys }),
  onSuccess: () => void queryClient.invalidateQueries({ queryKey: ['notifications'] }),
  meta: {
    error: 'notices.markFailed',
    success: (_data, keys) => (keys ? t('notices.markedRead') : t('notices.allRead')),
  },
})

/** What gets announced is chosen in the settings: the empty panel leads there. */
function openSettings() {
  open.value = false
  void router.push({ query: settingsQuery(router.currentRoute.value.query, 'notifications') })
}

function openNotice(record: NoticeRecord) {
  if (!record.read) markRead.mutate([record.key])
  const target = noticeRoute(record.link)
  if (!target) return
  open.value = false
  void router.push(target)
}
</script>

<template>
  <Button
    variant="ghost"
    size="icon"
    class="relative shrink-0"
    :aria-label="t('notices.title')"
    :title="t('notices.title')"
    data-tour="notification-bell"
    @click="open = true"
  >
    <Bell class="size-4" />
    <span
      v-if="unread"
      class="absolute right-1 top-1 grid min-w-4 place-items-center rounded-full bg-destructive px-1 text-[10px] font-semibold leading-4 text-destructive-foreground"
    >
      {{ unread > 99 ? '99+' : unread }}
    </span>
  </Button>

  <Sheet v-model:open="open">
    <SheetContent side="right" class="w-full gap-0 sm:max-w-md">
      <SheetHeader class="border-b border-border">
        <div class="flex items-center justify-between gap-2 pr-8">
          <SheetTitle>{{ t('notices.title') }}</SheetTitle>
          <Button v-if="unread" variant="ghost" size="sm" @click="markRead.mutate(null)">
            <CheckCheck class="size-4" />
            {{ t('notices.readAll') }}
          </Button>
        </div>
        <SheetDescription>{{ t('notices.hint') }}</SheetDescription>
      </SheetHeader>
      <div class="min-h-0 flex-1 overflow-y-auto p-3">
        <EmptyState v-if="!days.length" compact :icon="Bell" :title="t('notices.empty')" :description="t('notices.emptyHint')">
          <Button variant="outline" size="sm" @click="openSettings">{{ t('notices.openSettings') }}</Button>
        </EmptyState>
        <section v-for="[day, records] in days" :key="day" class="grid gap-1 pb-3">
          <p class="px-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">{{ formatDate(day) }}</p>
          <Button
            variant="ghost"
            size="block"
            v-for="record in records"
            :key="record.key"
            :class="
              cn(
                'grid gap-0.5 rounded-md px-3 py-2 text-left text-sm transition-colors hover:bg-accent',
                !record.read && 'bg-primary/5',
              )
            "
            @click="openNotice(record)"
          >
            <span class="flex items-center justify-between gap-2">
              <span class="flex items-center gap-1.5 font-medium">
                <span v-if="!record.read" class="size-1.5 shrink-0 rounded-full bg-primary" />
                {{ record.title }}
              </span>
              <span class="shrink-0 text-xs text-muted-foreground tabular-nums">{{ formatDateTime(record.delivered_at) }}</span>
            </span>
            <span class="text-muted-foreground">{{ record.body }}</span>
          </Button>
        </section>
      </div>
    </SheetContent>
  </Sheet>
</template>
