<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'

import { SharedEventSettlement } from '@/widgets/shared-event-settlement'
import { useSharedEvent } from '@/entities/shared-event'

import { EventHeader } from './ui/event-header'

/**
 * One shared event. For now only its settlement: the event screen with the plan, the purchases
 * and the participants is the next stream's and replaces this page.
 */
const route = useRoute()
const eventId = computed(() => String(route.params.id ?? ''))
const event = useSharedEvent(eventId)
</script>

<template>
  <div class="grid min-w-0 gap-4">
    <EventHeader :event="event.data.value?.event ?? null" />
    <SharedEventSettlement :event-id="eventId" />
  </div>
</template>
