import { type Ref, computed, ref, watch } from 'vue'

import { tickerRef } from '@/shared/lib/asset-ref'

import type { StoreCreateDialogProps, StoreCreateStep } from '../store-create.types'

/** «Для чего» on an exchange's sub-account (Wallets-2 · 9) and the type it stands for. */
export type ExchangeSubAim = 'futures' | 'earn' | 'other'
const AIM_TYPE: Record<ExchangeSubAim, SubType> = { futures: 'futures', earn: 'earn', other: 'funding' }

/**
 * An exchange's sub-account in one step of three fields: the name, what it is
 * for, the coin its margin is in. The kind and the exchange are known already,
 * so there is no type to pick first.
 */
export function useExchangeSub(
  props: StoreCreateDialogProps,
  open: Ref<boolean>,
  step: Ref<StoreCreateStep>,
  subType: Ref<SubType | null>,
) {
  const exchangeSub = computed(() => props.parent?.kind === 'exchange')
  const aim = ref<ExchangeSubAim>('futures')
  const margin = ref<AssetRef | null>(null)
  const withMargin = computed(() => exchangeSub.value && subType.value === 'futures')

  watch(aim, (value) => {
    if (exchangeSub.value) subType.value = AIM_TYPE[value]
  })
  // Registered after the dialog's own reset, so it has the last word on a fresh opening.
  watch(open, (isOpen) => {
    if (!isOpen || !exchangeSub.value) return
    aim.value = 'futures'
    subType.value = 'futures'
    margin.value = tickerRef('USDT')
    step.value = 'details'
  })

  return { exchangeSub, aim, margin, withMargin }
}
