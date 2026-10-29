<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { CircleHelp } from 'lucide-vue-next'

import { EVENT_STATUS_VARIANT, eventStatusKey } from '@/entities/shared-event'
import { formatDateRange } from '@/shared/lib/date-range'
import { ActionMenu } from '@/shared/ui/action-menu'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import type { UiAction } from '@/shared/ui/context-menu'

/** «Шашлыки · 2–4 ноя» with the status, the «⋯» of the event and «?» for the tour of this screen. */
const props = defineProps<{ view: SharedEventView; actions: UiAction[] }>()
const emit = defineEmits<{ tour: [] }>()
const { t, locale } = useI18n()

const dates = computed(() =>
  formatDateRange({ start: props.view.event.starts_on, end: props.view.event.ends_on }, locale.value),
)
</script>

<template>
  <div class="flex min-w-0 items-start gap-3">
    <div class="flex min-w-0 flex-1 flex-wrap items-center gap-x-3 gap-y-1">
      <h1 class="min-w-0 text-xl font-semibold [overflow-wrap:anywhere]">
        {{ props.view.event.title }}<template v-if="dates"> · {{ dates }}</template>
      </h1>
      <Badge :variant="EVENT_STATUS_VARIANT[props.view.status]" class="shrink-0" data-tour="shared-status">
        {{ t(eventStatusKey(props.view.status)) }}
      </Badge>
      <Badge v-if="props.view.event.archived" variant="outline" class="shrink-0">{{ t('sharedEvents.archivedBadge') }}</Badge>
    </div>
    <div class="flex shrink-0 items-center gap-1">
      <span class="inline-flex" data-tour="shared-event-menu">
        <ActionMenu :actions="props.actions" :label="t('actions.more')" :sheet="{ title: props.view.event.title }" />
      </span>
      <Button variant="outline" size="icon" :aria-label="t('tour.start')" :title="t('tour.start')" @click="emit('tour')">
        <CircleHelp class="size-4" />
      </Button>
    </div>
  </div>
</template>
