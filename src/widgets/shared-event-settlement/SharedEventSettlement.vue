<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Share } from 'lucide-vue-next'

import { SharedEventPayShareDialog } from '@/features/shared-event-pay-share'
import { SharedEventShareDialog } from '@/features/shared-event-share'
import { SharedDueDialog, SharedRemainderDialog } from '@/features/shared-remainder'
import { SharedSettleDialog } from '@/features/shared-settle'
import { EVENT_STATUS_VARIANT, eventStatusKey, sumDecimals, useSharedEvent } from '@/entities/shared-event'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { SectionState } from '@/shared/ui/section-state'

import { useSettlementActions } from './model/use-settlement-actions'
import type { SharedEventSettlementProps } from './shared-event-settlement.types'
import { MyPosition } from './ui/my-position'
import { SettlementTable } from './ui/settlement-table'

/**
 * The `Calculation` section of an event: my position, then who owes whom through the organizer, with
 * `received` / `Gave` / `Pay my share` on the rows and the rest in «⋯». It reads the event itself
 * (the shared query key), so every change anywhere refreshes it.
 */
const props = defineProps<SharedEventSettlementProps>()

const { t } = useI18n()
const event = useSharedEvent(() => props.eventId)
const view = computed(() => event.data.value)
const actions = useSettlementActions(view)

const organizerName = computed(() => view.value?.people.find((person) => person.is_organizer)?.name ?? '')
</script>

<template>
  <SectionState :loading="event.isPending.value" :error="event.error.value" skeleton="report" @retry="event.refetch()">
    <div v-if="view" class="grid min-w-0 gap-4">
      <Card data-tour="shared-settlement-summary">
        <div class="grid gap-4">
          <div class="flex flex-wrap items-center gap-2">
            <h2 class="text-base font-semibold">
              {{ view.mine ? t('sharedEvents.mine.title') : t('sharedEvents.settle.titleOthers') }}
            </h2>
            <Badge :variant="EVENT_STATUS_VARIANT[view.status]">{{ t(eventStatusKey(view.status)) }}</Badge>
            <div class="ml-auto flex items-center gap-2">
              <Button v-if="view.mine?.pay_to" data-tour="shared-pay-share" @click="actions.pay.value = true">
                {{ t('sharedEvents.settle.primary.pay') }}
              </Button>
              <Button variant="outline" data-tour="shared-share" @click="actions.share.value = true">
                <Share class="size-4" />
                {{ t('sharedEvents.share.open') }}
              </Button>
            </div>
          </div>
          <MyPosition
            v-if="view.mine"
            :mine="view.mine"
            :currency="view.event.currency"
            :decimals="sumDecimals(view)"
            :organizer-name="organizerName"
          />
          <p v-else class="text-sm text-muted-foreground">{{ t('sharedEvents.settle.notMine') }}</p>
        </div>
      </Card>

      <SettlementTable
        :view="view"
        :primary-of="actions.primaryOf"
        :actions-of="actions.actionsOf"
        @primary="actions.runPrimary"
      />

      <SharedSettleDialog
        v-model:open="actions.settle.open"
        :event="view"
        :person="actions.settlePerson.value"
        :kind="actions.settle.kind"
        :with-wallet="actions.withWallet.value"
      />
      <SharedRemainderDialog
        v-model:open="actions.remainder.open"
        :event="view"
        :person="actions.remainderPerson.value"
        :kind="actions.remainder.kind"
      />
      <SharedDueDialog v-model:open="actions.due.open" :event="view" :person="actions.duePerson.value" />
      <SharedEventPayShareDialog v-model:open="actions.pay.value" :event="view" />
      <SharedEventShareDialog v-model:open="actions.share.value" :event-id="view.event.id" :title="view.event.title" />
    </div>
  </SectionState>
</template>
