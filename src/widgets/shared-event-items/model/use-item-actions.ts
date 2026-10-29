import { type MaybeRefOrGetter, computed, reactive, ref, toValue } from 'vue'
import { useI18n } from 'vue-i18n'
import { Ban, Pencil, RotateCcw, Share2, ShoppingCart, Trash2, Undo2 } from 'lucide-vue-next'

import { ME, useDeleteSharedItem, useDeleteSharedPurchase, useUpdateSharedItem } from '@/entities/shared-event'
import type { UiAction } from '@/shared/ui/context-menu'

import type { ItemRow } from '../shared-event-items.types'

/**
 * What the items table can do: the «⋯» list of a row and the state of the dialogs it opens (one
 * item at a time, so one set of dialogs serves every row).
 */
export function useItemActions(view: MaybeRefOrGetter<SharedEventView | undefined>) {
  const { t } = useI18n()

  const itemDialog = reactive({ open: false, itemId: '' })
  const purchaseDialog = reactive({ open: false, itemId: '', purchaseId: '' })
  const splitDialog = reactive({ open: false, itemId: '' })
  const attachOpen = ref(false)
  const dropPurchase = reactive({ open: false, purchaseId: '', title: '' })

  const items = computed(() => toValue(view)?.event.items ?? [])
  const itemOf = (id: string) => items.value.find((item) => item.id === id) ?? null
  const editedItem = computed(() => itemOf(itemDialog.itemId))
  const boughtItem = computed(() => itemOf(purchaseDialog.itemId))
  const boughtPurchase = computed(
    () => boughtItem.value?.purchases.find((purchase) => purchase.id === purchaseDialog.purchaseId) ?? null,
  )
  const splitItem = computed(() => itemOf(splitDialog.itemId))

  const removeItem = useDeleteSharedItem()
  const removePurchase = useDeleteSharedPurchase()
  const updateItem = useUpdateSharedItem()

  const addItem = () => Object.assign(itemDialog, { open: true, itemId: '' })
  /** A purchase outside the plan: no item yet, the dialog asks for its name. */
  const addUnplanned = () => Object.assign(purchaseDialog, { open: true, itemId: '', purchaseId: '' })
  const buy = (row: ItemRow) => Object.assign(purchaseDialog, { open: true, itemId: row.item.id, purchaseId: '' })

  const buyerName = (id: string) =>
    id === ME ? t('sharedEvents.me') : (toValue(view)?.people.find((person) => person.id === id)?.name ?? '')

  function setCancelled(row: ItemRow, cancelled: boolean) {
    const event = toValue(view)
    if (!event) return
    const { item } = row
    updateItem.mutate({
      event_id: event.event.id,
      item_id: item.id,
      item: { title: item.title, unit: item.unit, category_id: item.category_id, plan: item.plan, cancelled },
    })
  }

  function confirmDropPurchase() {
    const event = toValue(view)
    if (!event) return
    removePurchase.mutate(
      { event_id: event.event.id, purchase_id: dropPurchase.purchaseId },
      { onSuccess: () => (dropPurchase.open = false) },
    )
  }

  /** «⋯» of an item: buy more, change or cancel a purchase, how to share, change, cancel, delete. */
  function actionsOf(row: ItemRow): UiAction[] {
    const event = toValue(view)
    if (!event) return []
    const { item } = row
    const bought = item.purchases.length > 0
    const many = item.purchases.length > 1
    const purchaseActions = item.purchases.flatMap((purchase): UiAction[] => {
      const who = many ? ` · ${buyerName(purchase.buyer)}` : ''
      return [
        {
          id: `edit-${purchase.id}`,
          label: t('sharedEvents.items.editPurchase') + who,
          icon: Pencil,
          hidden: item.cancelled,
          run: () => Object.assign(purchaseDialog, { open: true, itemId: item.id, purchaseId: purchase.id }),
        },
        {
          id: `drop-${purchase.id}`,
          label: t('sharedEvents.items.dropPurchase') + who,
          icon: Undo2,
          destructive: true,
          run: () => Object.assign(dropPurchase, { open: true, purchaseId: purchase.id, title: item.title }),
        },
      ]
    })
    return [
      {
        id: 'buy',
        label: bought ? t('sharedEvents.items.buyMore') : t('sharedEvents.items.buy'),
        icon: ShoppingCart,
        hidden: item.cancelled,
        run: () => buy(row),
      },
      ...purchaseActions.filter((entry) => !entry.destructive),
      {
        id: 'split',
        label: t('sharedEvents.items.split'),
        icon: Share2,
        hidden: item.cancelled,
        separated: true,
        run: () => Object.assign(splitDialog, { open: true, itemId: item.id }),
      },
      {
        id: 'edit',
        label: t('sharedEvents.items.edit'),
        icon: Pencil,
        run: () => Object.assign(itemDialog, { open: true, itemId: item.id }),
      },
      {
        id: 'cancel',
        label: item.cancelled ? t('sharedEvents.items.restore') : t('sharedEvents.items.cancel'),
        icon: item.cancelled ? RotateCcw : Ban,
        disabled: !item.cancelled && bought ? t('sharedEvents.items.cancelBlocked') : false,
        run: () => setCancelled(row, !item.cancelled),
      },
      ...purchaseActions.filter((entry) => entry.destructive),
      {
        id: 'delete',
        label: t('sharedEvents.items.delete'),
        icon: Trash2,
        destructive: true,
        separated: !bought,
        disabled: bought ? t('sharedEvents.items.deleteBlocked') : false,
        run: () =>
          removeItem.mutate({
            input: { event_id: event.event.id, item_id: item.id },
            title: item.title,
            draft: {
              title: item.title,
              unit: item.unit,
              category_id: item.category_id,
              plan: item.plan,
              cancelled: false,
            },
          }),
      },
    ]
  }

  return {
    itemDialog,
    purchaseDialog,
    splitDialog,
    attachOpen,
    dropPurchase,
    editedItem,
    boughtItem,
    boughtPurchase,
    splitItem,
    addItem,
    addUnplanned,
    buy,
    actionsOf,
    confirmDropPurchase,
    dropping: removePurchase.isPending,
  }
}
