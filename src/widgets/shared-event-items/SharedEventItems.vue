<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Link2, Plus, ShoppingBasket, ShoppingCart } from 'lucide-vue-next'

import { AttachPaymentDialog, SharedPurchaseDialog } from '@/features/shared-purchase-add'
import { SharedItemEditDialog } from '@/features/shared-item-edit'
import { SharedItemSplitDialog } from '@/features/shared-item-split'
import { sumDecimals, useSharedEvent } from '@/entities/shared-event'
import { Button } from '@/shared/ui/button'
import { ConfirmDialog } from '@/shared/ui/confirm-dialog'
import { SectionState } from '@/shared/ui/section-state'

import { useItemActions } from './model/use-item-actions'
import { useItemRows } from './model/use-item-rows'
import type { SharedEventItemsProps } from './shared-event-items.types'
import { ItemTable } from './ui/item-table'

/**
 * The «Позиции» section: the plan and the purchases in one list. Add an item, mark it bought,
 * share it out («Как делить»), change or cancel it; a purchase outside the plan and an existing
 * payment can be added too. Reads the event itself (the shared query key), so any change refreshes it.
 */
const props = defineProps<SharedEventItemsProps>()
const { t } = useI18n()

const event = useSharedEvent(() => props.eventId)
const view = computed(() => event.data.value)
const rows = useItemRows(view)
const actions = useItemActions(view)
const decimals = computed(() => (view.value ? sumDecimals(view.value) : 2))
</script>

<template>
  <div class="min-w-0">
  <SectionState
    :loading="event.isPending.value"
    :error="event.error.value"
    :empty="Boolean(view) && !rows.length"
    :empty-title="t('sharedEvents.items.emptyTitle')"
    :empty-description="t('sharedEvents.items.emptyHint')"
    :empty-icon="ShoppingBasket"
    skeleton="table"
    @retry="event.refetch()"
  >
    <template #empty>
      <div class="flex flex-wrap justify-center gap-2">
        <Button data-tour="shared-add-item" @click="actions.addItem()"><Plus class="size-4" />{{ t('sharedEvents.items.add') }}</Button>
        <Button variant="outline" @click="actions.addUnplanned()"><ShoppingCart class="size-4" />{{ t('sharedEvents.items.addBought') }}</Button>
      </div>
    </template>

    <div v-if="view" class="grid min-w-0 gap-3">
      <div class="flex flex-wrap items-center gap-2">
        <Button data-tour="shared-add-item" @click="actions.addItem()"><Plus class="size-4" />{{ t('sharedEvents.items.add') }}</Button>
        <Button variant="outline" data-tour="shared-add-bought" @click="actions.addUnplanned()">
          <ShoppingCart class="size-4" />{{ t('sharedEvents.items.addBought') }}
        </Button>
        <Button v-if="view.event.i_participate" variant="ghost" class="ml-auto" @click="actions.attachOpen.value = true">
          <Link2 class="size-4" />{{ t('sharedEvents.items.attach') }}
        </Button>
      </div>
      <ItemTable :rows="rows" :currency="view.event.currency" :decimals="decimals" :actions-of="actions.actionsOf" @buy="actions.buy" />
    </div>

  </SectionState>
    <template v-if="view">
      <SharedItemEditDialog v-model:open="actions.itemDialog.open" :event="view" :item="actions.editedItem.value" />
      <SharedPurchaseDialog
        v-model:open="actions.purchaseDialog.open"
        :event="view"
        :item="actions.boughtItem.value"
        :purchase="actions.boughtPurchase.value"
      />
      <SharedItemSplitDialog v-model:open="actions.splitDialog.open" :event="view" :item="actions.splitItem.value" />
      <AttachPaymentDialog v-model:open="actions.attachOpen.value" :event="view" />
      <ConfirmDialog
        v-model:open="actions.dropPurchase.open"
        overlay="shared-drop-purchase"
        :title="t('sharedEvents.items.dropTitle', { title: actions.dropPurchase.title })"
        :description="t('sharedEvents.items.dropText')"
        :confirm-label="t('sharedEvents.items.dropConfirm')"
        :pending="actions.dropping.value"
        @confirm="actions.confirmDropPurchase()"
      />
    </template>
  </div>
</template>
