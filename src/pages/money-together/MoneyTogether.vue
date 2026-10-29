<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { Plus, Users } from 'lucide-vue-next'

import { useSharedEvents } from '@/entities/shared-event'
import { SharedEventDialog } from '@/widgets/shared-event-dialog'
import { Button } from '@/shared/ui/button'
import { Label } from '@/shared/ui/label'
import { ListHeader } from '@/shared/ui/list-header'
import { SectionState } from '@/shared/ui/section-state'
import { Switch } from '@/shared/ui/switch'

import { EventRow } from './ui/event-row'

/**
 * «Вместе»: the events — посиделки, trips, gifts — where several people spend and settle up. A new
 * event opens its screen; an old closed one can go to the archive and is shown on request.
 */
const { t } = useI18n()
const router = useRouter()

const archived = ref(false)
const events = useSharedEvents(archived)
const rows = computed(() => events.data.value ?? [])
const createOpen = ref(false)

function opened(view: SharedEventView) {
  void router.push({ name: 'money-together-event', params: { id: view.event.id } })
}
</script>

<template>
  <div class="grid min-w-0 gap-4">
    <ListHeader
      :intro="t('sharedEvents.list.intro')"
      hide-action
    >
      <template #actions>
        <div class="flex items-center gap-2" data-tour="shared-archive">
          <Switch id="shared-archived" v-model="archived" />
          <Label for="shared-archived" class="text-sm font-normal">{{ t('sharedEvents.list.archive') }}</Label>
        </div>
        <Button v-if="rows.length || archived" data-tour="shared-new" @click="createOpen = true">
          <Plus class="size-4" />{{ t('sharedEvents.list.new') }}
        </Button>
      </template>
    </ListHeader>

    <SectionState
      :loading="events.isPending.value"
      :error="events.error.value"
      :empty="!rows.length"
      :empty-title="archived ? t('sharedEvents.list.emptyArchiveTitle') : t('sharedEvents.list.emptyTitle')"
      :empty-description="archived ? t('sharedEvents.list.emptyArchiveHint') : t('sharedEvents.list.emptyHint')"
      :empty-icon="Users"
      skeleton="list"
      @retry="events.refetch()"
    >
      <template #empty>
        <Button v-if="!archived" data-tour="shared-new" @click="createOpen = true">
          <Plus class="size-4" />{{ t('sharedEvents.list.new') }}
        </Button>
      </template>
      <ul class="grid gap-2" data-tour="shared-list">
        <li v-for="event in rows" :key="event.id"><EventRow :event="event" /></li>
      </ul>
    </SectionState>

    <SharedEventDialog v-model:open="createOpen" @saved="opened" />
  </div>
</template>
