import { type Ref, computed, ref, watch } from 'vue'

import { ME, useAddSharedPurchase, useLastWallet, useUpdateSharedPurchase } from '@/entities/shared-event'
import { decimalMul, parseDecimal } from '@/shared/lib/decimal'
import { positiveAmount, required, useFormValidation } from '@/shared/lib/form'
import { dateInputToIso, localDate } from '@/shared/lib/planning'

import type { SharedPurchaseDialogProps } from '../shared-purchase-add.types'
import { defaultPurchaseDate } from './purchase-date'

/** The fields and the save of `Bought` (new, or a change of a purchase). */
export function usePurchaseForm(open: Ref<boolean>, props: SharedPurchaseDialogProps) {
  const event = computed(() => props.event.event)

  const buyer = ref(ME)
  const qty = ref('')
  const amount = ref('')
  const currency = ref('')
  const rate = ref('')
  const date = ref('')
  const title = ref('')
  const unit = ref('шт')

  /** A payment that existed before the event took it: only the quantity may change. */
  const linked = computed(() => Boolean(props.purchase?.tx_id && !props.purchase.tx_owned))
  const isNew = computed(() => !props.item && !props.purchase)
  const mine = computed(() => buyer.value === ME)
  const walletNeeded = computed(() => mine.value && !linked.value)
  const wallet = useLastWallet(open, walletNeeded)

  const foreign = computed(() => currency.value.toLowerCase() !== event.value.currency.toLowerCase())
  const planView = computed(() => props.event.items.find((entry) => entry.item_id === props.item?.id))

  watch(open, (now) => {
    if (!now) return
    const own = props.purchase
    buyer.value =
      own?.buyer ?? (event.value.i_participate ? ME : (props.event.people.find((person) => person.id !== ME)?.id ?? ME))
    qty.value = own?.qty ?? props.item?.plan?.qty ?? '1'
    const total = own
      ? own.price_mode === 'per_unit'
        ? (decimalMul(own.qty, own.amount) ?? own.amount)
        : own.amount
      : (planView.value?.plan_total ?? '')
    amount.value = total
    currency.value = (own?.currency ?? event.value.currency).toLowerCase()
    rate.value = own?.rate_manual ? own.rate_to_event : ''
    date.value = own
      ? own.at.slice(0, 10)
      : defaultPurchaseDate(localDate(new Date()), event.value.starts_on, event.value.ends_on)
    title.value = ''
    unit.value = props.item?.unit ?? 'шт'
  })

  const form = useFormValidation(
    () => ({
      title: isNew.value ? required(title.value) : null,
      qty: positiveAmount(qty.value),
      amount: linked.value ? null : positiveAmount(amount.value),
      currency: required(currency.value, 'pick'),
      rate: foreign.value && rate.value.trim() ? positiveAmount(rate.value) : null,
      store: walletNeeded.value ? required(wallet.storeId.value, 'pick') : null,
      buyer: required(buyer.value),
    }),
    { resetOn: open },
  )

  /** `Price per pc 225 ₽`: what one unit came to, once both numbers are in. */
  const unitPriceHint = computed(() => {
    const count = Number(parseDecimal(qty.value) ?? 0)
    const sum = Number(parseDecimal(amount.value) ?? 0)
    if (!(count > 0) || !(sum > 0)) return null
    return String(Math.round((sum / count) * 100) / 100)
  })

  const add = useAddSharedPurchase()
  const change = useUpdateSharedPurchase()
  const pending = computed(() => add.isPending.value || change.isPending.value)

  function save() {
    const draft: SharedPurchaseDraft = {
      item_id: props.item?.id ?? null,
      new_item_title: isNew.value ? title.value.trim() : null,
      new_item_unit: isNew.value ? unit.value.trim() || 'шт' : null,
      buyer: buyer.value,
      qty: parseDecimal(qty.value) ?? qty.value,
      amount: parseDecimal(amount.value) ?? amount.value,
      currency: currency.value,
      price_mode: 'total',
      rate_to_event: foreign.value && rate.value.trim() ? (parseDecimal(rate.value) ?? rate.value) : null,
      at: dateInputToIso(date.value) ? date.value : null,
      store_id: mine.value && !linked.value ? wallet.storeId.value : null,
    }
    const done = {
      onSuccess: () => {
        if (draft.store_id) wallet.remember()
        open.value = false
      },
    }
    const eventId = event.value.id
    if (props.purchase) {
      change.mutate({ event_id: eventId, purchase_id: props.purchase.id, purchase: draft }, done)
      return
    }
    add.mutate(
      {
        input: { event_id: eventId, purchase: draft },
        knownIds: props.event.event.items.flatMap((entry) => entry.purchases.map((purchase) => purchase.id)),
        title: props.item?.title ?? title.value.trim(),
        walletName: draft.store_id ? wallet.name.value : undefined,
        currency: currency.value,
      },
      done,
    )
  }

  return {
    buyer,
    qty,
    amount,
    currency,
    rate,
    date,
    title,
    unit,
    linked,
    isNew,
    mine,
    foreign,
    wallet,
    form,
    unitPriceHint,
    pending,
    save,
  }
}
