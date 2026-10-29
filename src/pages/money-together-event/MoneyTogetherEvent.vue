<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRoute, useRouter } from 'vue-router'

import { useSharedEvent } from '@/entities/shared-event'
import { SharedEventDialog } from '@/widgets/shared-event-dialog'
import { SharedEventItems } from '@/widgets/shared-event-items'
import { SharedEventPeople } from '@/widgets/shared-event-people'
import { SharedEventSettlement } from '@/widgets/shared-event-settlement'
import { useTour } from '@/shared/lib/tour/use-tour'
import { Card } from '@/shared/ui/card'
import { ConfirmDialog } from '@/shared/ui/confirm-dialog'
import { SectionState } from '@/shared/ui/section-state'
import { Tabs } from '@/shared/ui/tabs'

import { useEventMenu } from './model/use-event-menu'
import { EventHeader } from './ui/event-header'
import { EventTotals } from './ui/event-totals'

/**
 * One shared event: the header with the totals and the shares, then three sections — the items
 * (plan and purchases), the people, the settlement. The section is kept in the address (`?tab=`).
 */
const route = useRoute()
const router = useRouter()
const { t } = useI18n()
const tour = useTour()

const eventId = computed(() => String(route.params.id ?? ''))
const event = useSharedEvent(eventId)
const view = computed(() => event.data.value)
const menu = useEventMenu(view)

const TABS = ['items', 'people', 'settle'] as const
const tab = computed({
  get: () => (TABS.includes(route.query.tab as (typeof TABS)[number]) ? String(route.query.tab) : 'items'),
  set: (value: string) => void router.replace({ query: { ...route.query, tab: value } }),
})
const tabs = computed(() => TABS.map((value) => ({ value, label: t(`sharedEvents.tabs.${value}`) })))
</script>

<template>
  <div class="grid min-w-0 gap-4">
    <SectionState :loading="event.isPending.value" :error="event.error.value" skeleton="report" @retry="event.refetch()">
      <template v-if="view">
        <Card>
          <div class="grid gap-4">
            <EventHeader :view="view" :actions="menu.actions()" @tour="tour.start('money-together-event')" />
            <EventTotals :view="view" />
          </div>
        </Card>
        <div data-tour="shared-tabs">
          <Tabs v-model="tab" :tabs="tabs">
            <template #items><SharedEventItems :event-id="eventId" /></template>
            <template #people><SharedEventPeople :event-id="eventId" /></template>
            <template #settle><SharedEventSettlement :event-id="eventId" /></template>
          </Tabs>
        </div>
        <SharedEventDialog v-model:open="menu.editOpen.value" :event="view.event" />
        <ConfirmDialog
          v-model:open="menu.dropOpen.value"
          overlay="shared-event-drop"
          :title="t('sharedEvents.menu.dropTitle', { title: view.event.title })"
          :description="t('sharedEvents.menu.dropText')"
          :pending="menu.deleting.value"
          @confirm="menu.confirmDelete()"
        />
      </template>
    </SectionState>
  </div>
</template>
