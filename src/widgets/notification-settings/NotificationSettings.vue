<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { ChevronDown } from 'lucide-vue-next'

import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/shared/ui/collapsible'
import { SwitchRow } from '@/shared/ui/switch-row'

import NotificationDetails from './NotificationDetails.vue'
import { useNotificationConfig } from './use-notification-config'

/**
 * `Notifications`: three switches for the kinds people think in; the rest
 * (channel, time, per-category lead days, permission) sits under `Detailed settings`.
 */
const { t } = useI18n()
const { category, patchCategory } = useNotificationConfig()

const BASIC: { id: string; label: string; category: NoticeCategory }[] = [
  { id: 'notice-reminders', label: 'notices.basic.reminders', category: 'reminder' },
  { id: 'notice-price-alerts', label: 'notices.basic.priceAlerts', category: 'price_alert' },
  { id: 'notice-automations', label: 'notices.basic.automations', category: 'automation' },
]
</script>

<template>
  <div class="flex flex-col gap-4" data-tour="notification-settings">
    <SwitchRow
      plain
      v-for="item in BASIC"
      :id="item.id"
      :key="item.id"
      :label="t(item.label)"
      :model-value="category(item.category).enabled"
      @update:model-value="(value: boolean) => patchCategory(item.category, { enabled: value })"
    />
    <Collapsible class="grid gap-3">
      <CollapsibleTrigger class="group flex min-h-10 w-fit items-center gap-1 text-sm text-primary hover:underline">
        {{ t('notices.moreTitle') }}
        <ChevronDown class="size-4 transition-transform group-data-[state=open]:rotate-180" />
      </CollapsibleTrigger>
      <CollapsibleContent>
        <NotificationDetails />
      </CollapsibleContent>
    </Collapsible>
  </div>
</template>
