<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { Ellipsis, Users } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { eventStatusKey, useSharedEvents } from '@/entities/shared-event'
import { partsText } from '@/shared/lib/money-parts'
import {
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarMenu,
  SidebarMenuBadge,
  SidebarMenuButton,
  SidebarMenuItem,
} from '@/shared/ui/sidebar'

import { SIDEBAR_EVENTS_LIMIT, activeEvents, eventBalance } from './lib/active-events'

/**
 * «Events»: the shared events still going, under the favourites, with my
 * balance in each. Hidden while loading and while there are none. Shares the
 * list's query, so every change of an event refreshes it.
 */
const { t } = useI18n()
const route = useRoute()
const events = useSharedEvents(false)

const active = computed(() => activeEvents(events.data.value ?? []))
const rows = computed(() =>
  active.value.slice(0, SIDEBAR_EVENTS_LIMIT).map((event) => {
    const balance = eventBalance(event)
    const status = t(eventStatusKey(event.status))
    const amount = balance ? partsText([{ amount: balance, currency: event.currency, signed: true }]) : ''
    return {
      id: event.id,
      title: event.title,
      balance,
      currency: event.currency,
      tooltip: [`${event.title} · ${status}`, amount].filter(Boolean).join(' · '),
      current: route.name === 'money-together-event' && route.params.id === event.id,
    }
  }),
)
const more = computed(() => active.value.length > SIDEBAR_EVENTS_LIMIT)
const allLabel = computed(() => t('sidebar.eventsAll', { count: active.value.length }))
</script>

<template>
  <SidebarGroup v-if="rows.length" class="py-1" data-tour="sidebar-events">
    <SidebarGroupLabel class="h-6 group-data-[collapsible=icon]:-mt-6">{{ t('sidebar.events') }}</SidebarGroupLabel>
    <SidebarGroupContent>
      <SidebarMenu>
        <SidebarMenuItem v-for="row in rows" :key="row.id">
          <SidebarMenuButton as-child :is-active="row.current" :tooltip="row.tooltip">
            <RouterLink :to="{ name: 'money-together-event', params: { id: row.id } }">
              <Users />
              <span>{{ row.title }}</span>
            </RouterLink>
          </SidebarMenuButton>
          <SidebarMenuBadge v-if="row.balance" class="right-1 tabular-nums">
            <Money :amount="row.balance" :currency="row.currency" signed colored />
          </SidebarMenuBadge>
        </SidebarMenuItem>
        <SidebarMenuItem v-if="more">
          <SidebarMenuButton as-child :tooltip="allLabel" class="text-muted-foreground">
            <RouterLink :to="{ name: 'money-together' }">
              <Ellipsis />
              <span>{{ allLabel }}</span>
            </RouterLink>
          </SidebarMenuButton>
        </SidebarMenuItem>
      </SidebarMenu>
    </SidebarGroupContent>
  </SidebarGroup>
</template>
