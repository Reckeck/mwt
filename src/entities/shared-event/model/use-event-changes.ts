import { useMutation } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { i18n } from '@/shared/config/i18n'
import { formatMoney } from '@/shared/lib/format'

import type { ArchiveChange, PurchaseChange, SplitChange } from './event-changes.types'
import { useSharedEventSync } from './use-shared-event-sync'

const translate = (key: string, params?: Record<string, unknown>) => i18n.global.t(key, params ?? {})

const run = (command: string, input: unknown) => call<SharedEventView>(command, { input })

/** Every purchase id in the answer: the new one is the id that was not known before. */
function purchaseIds(view: SharedEventView): string[] {
  return view.event.items.flatMap((item) => item.purchases.map((purchase) => purchase.id))
}

/** «Новое событие»: the event with the people typed in the dialog; the toast offers «Отменить». */
export function useCreateSharedEvent() {
  const sync = useSharedEventSync()
  return useMutation({
    mutationFn: (draft: SharedEventDraft) => run('create_shared_event', draft),
    onSuccess: sync,
    meta: {
      error: 'sharedEvents.toast.createFailed',
      success: (data) => translate('sharedEvents.toast.created', { title: (data as SharedEventView).event.title }),
      undo: (data) => call<void>('delete_shared_event', { id: (data as SharedEventView).event.id }),
    },
  })
}

export function useUpdateSharedEvent() {
  const sync = useSharedEventSync()
  return useMutation({
    mutationFn: (input: UpdateSharedEventInput) => run('update_shared_event', input),
    onSuccess: sync,
    meta: { error: 'sharedEvents.toast.saveFailed', success: 'sharedEvents.toast.saved' },
  })
}

/** The archive: closed events only; «Отменить» brings it back. */
export function useArchiveSharedEvent() {
  const sync = useSharedEventSync()
  return useMutation({
    mutationFn: (change: ArchiveChange) =>
      call<void>('archive_shared_event', { input: { id: change.id, archived: change.archived } }),
    onSuccess: sync,
    meta: {
      error: 'sharedEvents.toast.archiveFailed',
      success: (_data, variables) => {
        const change = variables as ArchiveChange
        return translate(change.archived ? 'sharedEvents.toast.archived' : 'sharedEvents.toast.unarchived', {
          title: change.title,
        })
      },
      undo: (_data, variables) => {
        const change = variables as ArchiveChange
        return call<void>('archive_shared_event', { input: { id: change.id, archived: !change.archived } })
      },
    },
  })
}

/** Delete the event; its real operations stay in the vault (a `ConfirmButton` asks first). */
export function useDeleteSharedEvent() {
  const sync = useSharedEventSync()
  return useMutation({
    mutationFn: (change: { id: string; title: string }) => call<void>('delete_shared_event', { id: change.id }),
    onSuccess: sync,
    meta: {
      error: 'sharedEvents.toast.deleteFailed',
      success: (_data, variables) =>
        translate('sharedEvents.toast.deleted', { title: (variables as { title: string }).title }),
    },
  })
}

export function useAddSharedParticipant() {
  const sync = useSharedEventSync()
  return useMutation({
    mutationFn: (input: SharedParticipantInput) => run('add_shared_participant', input),
    onSuccess: sync,
    meta: {
      error: 'sharedEvents.toast.personFailed',
      success: (_data, variables) =>
        translate('sharedEvents.toast.personAdded', {
          name: (variables as SharedParticipantInput).participant.name,
        }),
    },
  })
}

export function useUpdateSharedParticipant() {
  const sync = useSharedEventSync()
  return useMutation({
    mutationFn: (input: UpdateSharedParticipantInput) => run('update_shared_participant', input),
    onSuccess: sync,
    meta: { error: 'sharedEvents.toast.personFailed', success: 'sharedEvents.toast.personSaved' },
  })
}

export function useRemoveSharedParticipant() {
  const sync = useSharedEventSync()
  return useMutation({
    mutationFn: (change: { input: SharedParticipantRef; name: string }) =>
      run('remove_shared_participant', change.input),
    onSuccess: sync,
    meta: {
      error: 'sharedEvents.toast.personFailed',
      success: (_data, variables) =>
        translate('sharedEvents.toast.personRemoved', { name: (variables as { name: string }).name }),
    },
  })
}

/** «Сделать контактом»: the guest's details become a contact and the debts move to its card. */
export function useMakeSharedParticipantContact() {
  const sync = useSharedEventSync()
  return useMutation({
    mutationFn: (change: { input: SharedParticipantRef; name: string }) =>
      call<Contact>('make_shared_participant_contact', { input: change.input }),
    onSuccess: sync,
    meta: {
      error: 'sharedEvents.toast.personFailed',
      success: (_data, variables) =>
        translate('sharedEvents.toast.personContact', { name: (variables as { name: string }).name }),
    },
  })
}

export function useAddSharedItem() {
  const sync = useSharedEventSync()
  return useMutation({
    mutationFn: (input: SharedItemInput) => run('add_shared_item', input),
    onSuccess: sync,
    meta: {
      error: 'sharedEvents.toast.itemFailed',
      success: (_data, variables) =>
        translate('sharedEvents.toast.itemAdded', { title: (variables as SharedItemInput).item.title }),
    },
  })
}

export function useUpdateSharedItem() {
  const sync = useSharedEventSync()
  return useMutation({
    mutationFn: (input: UpdateSharedItemInput) => run('update_shared_item', input),
    onSuccess: sync,
    meta: { error: 'sharedEvents.toast.itemFailed', success: 'sharedEvents.toast.itemSaved' },
  })
}

export function useDeleteSharedItem() {
  const sync = useSharedEventSync()
  return useMutation({
    mutationFn: (change: { input: SharedItemRef; title: string; draft: SharedItemDraft }) =>
      run('delete_shared_item', change.input),
    onSuccess: sync,
    meta: {
      error: 'sharedEvents.toast.itemFailed',
      success: (_data, variables) =>
        translate('sharedEvents.toast.itemDeleted', { title: (variables as { title: string }).title }),
      // The plan line comes back (a new id; an item without purchases has nothing else to lose).
      undo: (_data, variables) =>
        run('add_shared_item', {
          event_id: (variables as { input: SharedItemRef }).input.event_id,
          item: (variables as { draft: SharedItemDraft }).draft,
        }),
    },
  })
}

/** «Как делить»: the shares are recalculated at once; «Отменить» puts the old split back. */
export function useSetSharedItemSplit() {
  const sync = useSharedEventSync()
  return useMutation({
    mutationFn: (change: SplitChange) => run('set_shared_item_split', change.input),
    onSuccess: sync,
    meta: {
      error: 'sharedEvents.toast.splitFailed',
      success: 'sharedEvents.toast.splitSaved',
      undo: (_data, variables) =>
        run('set_shared_item_split', {
          ...(variables as SplitChange).input,
          split: (variables as SplitChange).previous,
        }),
    },
  })
}

/** A hand-set total of one person; empty `total` takes it off. */
export function useSetSharedOverride() {
  const sync = useSharedEventSync()
  return useMutation({
    mutationFn: (input: SetShareOverrideInput) => run('set_shared_share_override', input),
    onSuccess: sync,
    meta: {
      error: 'sharedEvents.toast.overrideFailed',
      success: (_data, variables) =>
        translate(
          (variables as SetShareOverrideInput).total === null
            ? 'sharedEvents.toast.overrideCleared'
            : 'sharedEvents.toast.overrideSet',
        ),
    },
  })
}

/** «Купили»: a purchase; mine makes a payment, so the toast says from which wallet. */
export function useAddSharedPurchase() {
  const sync = useSharedEventSync()
  return useMutation({
    mutationFn: (change: PurchaseChange) => run('add_shared_purchase', change.input),
    onSuccess: sync,
    meta: {
      error: 'sharedEvents.toast.purchaseFailed',
      success: (_data, variables) => {
        const change = variables as PurchaseChange
        // money: plain — the toast is text
        const amount = formatMoney(change.input.purchase.amount, { currency: change.currency })
        return change.walletName
          ? translate('sharedEvents.toast.purchasePaid', { amount, wallet: change.walletName })
          : translate('sharedEvents.toast.purchaseAdded', { title: change.title })
      },
      undo: (data, variables) => {
        const change = variables as PurchaseChange
        const fresh = purchaseIds(data as SharedEventView).find((id) => !change.knownIds.includes(id))
        if (!fresh) return undefined
        return run('delete_shared_purchase', { event_id: change.input.event_id, purchase_id: fresh })
      },
    },
  })
}

export function useUpdateSharedPurchase() {
  const sync = useSharedEventSync()
  return useMutation({
    mutationFn: (input: UpdateSharedPurchaseInput) => run('update_shared_purchase', input),
    onSuccess: sync,
    meta: { error: 'sharedEvents.toast.purchaseFailed', success: 'sharedEvents.toast.purchaseSaved' },
  })
}

/** Cancel a purchase: a payment the event made goes with it, a linked one stays. */
export function useDeleteSharedPurchase() {
  const sync = useSharedEventSync()
  return useMutation({
    mutationFn: (input: { event_id: string; purchase_id: string }) => run('delete_shared_purchase', input),
    onSuccess: sync,
    meta: { error: 'sharedEvents.toast.purchaseFailed', success: 'sharedEvents.toast.purchaseDeleted' },
  })
}

/** «Привязать платёж»: an existing payment of mine becomes a purchase of the event. */
export function useAttachSharedPayment() {
  const sync = useSharedEventSync()
  return useMutation({
    mutationFn: (input: AttachPaymentInput) => run('attach_payment_to_shared_event', input),
    onSuccess: sync,
    meta: { error: 'sharedEvents.toast.attachFailed', success: 'sharedEvents.toast.attached' },
  })
}
