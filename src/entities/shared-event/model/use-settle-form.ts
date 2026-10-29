import { type MaybeRefOrGetter, type Ref, computed, ref, toValue, watch } from 'vue'

import { i18n } from '@/shared/config/i18n'
import { useStoreTree } from '@/shared/api/queries'
import { firstError, positiveAmount, required, useFormValidation } from '@/shared/lib/form'
import { formatMoney } from '@/shared/lib/format'
import { localDate } from '@/shared/lib/planning'
import { flattenStoreTree } from '@/shared/lib/store-tree'

import { absolute, exceeds, settleDirection, sumDecimals } from '../lib/settlement'

import type { SettleKind } from './settle.types'
import { useSettleShared } from './use-settle-shared'

const LAST_WALLET_KEY = 'mwt:shared-settle-wallet'

function rememberedWallet(): string {
  try {
    return localStorage.getItem(LAST_WALLET_KEY) ?? ''
  } catch {
    return ''
  }
}

function rememberWallet(id: string) {
  try {
    localStorage.setItem(LAST_WALLET_KEY, id)
  } catch {
    // Private window or blocked storage: the next form simply starts on the first wallet.
  }
}

export interface SettleFormOptions {
  event: MaybeRefOrGetter<SharedEventView | undefined>
  /** Whose debt it is: the person, or me when I pay my share. */
  person: MaybeRefOrGetter<PersonView | undefined>
  open: Ref<boolean>
  /** Money of mine moves, so a wallet is chosen; without it the settlement is a set-off. */
  withWallet: MaybeRefOrGetter<boolean>
  kind: MaybeRefOrGetter<SettleKind>
  /** Who the toast names: the person, or the organizer when I pay. */
  name: MaybeRefOrGetter<string>
}

/**
 * The fields and the save of one settlement form (`Received`, `Gave`, `Pay the share`, `Offset`):
 * the sum starts at what is owed and may be less, never more; the wallet starts at the last one used.
 */
export function useSettleForm(options: SettleFormOptions) {
  const amount = ref('')
  const storeId = ref('')
  const date = ref(localDate(new Date()))

  const owed = computed(() => absolute(toValue(options.person)?.saldo ?? '0'))
  const currency = computed(() => toValue(options.event)?.event.currency ?? '')
  const decimals = computed(() => {
    const view = toValue(options.event)
    return view ? sumDecimals(view) : 2
  })
  const direction = computed(() => settleDirection(toValue(options.person)?.saldo ?? '0') ?? 'to_organizer')
  const walletNeeded = computed(() => toValue(options.withWallet))

  const tree = useStoreTree(computed(() => options.open.value && walletNeeded.value))
  const nodes = computed(() => (tree.data.value ?? []).filter((node) => !node.store.archived))
  const flat = computed(() => flattenStoreTree(nodes.value))

  // Pick the wallet once the list is there: the last used one, else the first.
  watch(
    [() => options.open.value, flat],
    ([open, list]) => {
      if (!open || !walletNeeded.value || list.some((node) => node.store.id === storeId.value)) return
      const remembered = rememberedWallet()
      storeId.value = list.find((node) => node.store.id === remembered)?.store.id ?? list[0]?.store.id ?? ''
    },
    { immediate: true },
  )

  const form = useFormValidation(() => ({
    amount: firstError(
      positiveAmount(amount.value),
      exceeds(amount.value || '0', owed.value)
        ? i18n.global.t('sharedEvents.settle.tooMuch', {
            // money: plain — the message under the field is text
            max: formatMoney(owed.value, { currency: currency.value, decimals: decimals.value }),
          })
        : null,
    ),
    store: walletNeeded.value ? required(storeId.value) : null,
  }))

  watch(
    () => options.open.value,
    (open) => {
      if (!open) return
      amount.value = owed.value
      date.value = localDate(new Date())
      form.reset()
    },
    { immediate: true },
  )
  const mutation = useSettleShared()

  function save() {
    const target = toValue(options.person)
    if (!target) return
    const wallet = walletNeeded.value ? flat.value.find((node) => node.store.id === storeId.value) : undefined
    mutation.mutate(
      {
        input: {
          event_id: toValue(options.event)?.event.id ?? '',
          participant_id: target.id,
          direction: direction.value,
          amount: amount.value,
          store_id: wallet ? wallet.store.id : null,
          at: date.value || null,
        },
        kind: toValue(options.kind),
        name: toValue(options.name),
        currency: currency.value,
        decimals: decimals.value,
        walletName: wallet?.store.name,
      },
      {
        onSuccess: () => {
          if (wallet) rememberWallet(wallet.store.id)
          options.open.value = false
        },
      },
    )
  }

  return {
    amount,
    storeId,
    date,
    nodes,
    owed,
    currency,
    decimals,
    form,
    pending: computed(() => mutation.isPending.value),
    save,
  }
}
