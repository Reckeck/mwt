import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { connectBybit } from '@/features/exchange-key'
import { call } from '@/shared/api/ipc'
import { notify } from '@/shared/lib/notify'
import { useSettingsStore } from '@/shared/store/settings'

import type { StoreCreateState } from './use-store-create-state'

/** The new wallet as the backend input, from what the dialog holds. */
function createInput(state: StoreCreateState, baseCurrency: string): CreateStoreInput {
  const { props, isSub, kind, subInfo } = state
  return {
    name: state.name.value.trim(),
    kind: kind.value,
    color: state.color.value,
    note: '',
    parent_id: props.parent?.id ?? null,
    sub_type: isSub.value ? state.subType.value : null,
    binding: isSub.value ? state.binding.value : null,
    purpose: null,
    liability: kind.value === 'liability' || subInfo.value?.credit ? state.terms.value : null,
    contact_id: null,
    custody: !isSub.value && kind.value === 'crypto' ? state.custody.value : null,
    property:
      !isSub.value && kind.value === 'property'
        ? {
            category: state.propertyCategory.value,
            address: '',
            area: null,
            purchased_at: null,
            purchase_price: null,
            currency: baseCurrency.toLowerCase(),
          }
        : null,
    secured_by: kind.value === 'liability' && state.securedBy.value ? state.securedBy.value : null,
    provider: !isSub.value && kind.value === 'exchange' ? state.provider.value : null,
    opening:
      state.offersOpening.value && state.openingAmountValue.value && state.openingAsset.value
        ? { asset: state.openingAsset.value, amount: state.openingAmountValue.value }
        : null,
    card: state.withCard.value ? { name: state.cardName.value.trim(), currency: state.cardCurrency.value } : null,
  }
}

/**
 * Creating the wallet; an exchange given a Bybit key connects right after —
 * a failed key leaves the wallet in place, the key can be added in settings.
 */
export function useCreateStore(state: StoreCreateState, onCreated: (store: Store) => void) {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  const settings = useSettingsStore()

  return useMutation({
    mutationFn: async () => {
      const store = await call<Store>('create_store', { input: createInput(state, settings.baseCurrency) })
      if (!state.isSub.value && state.kind.value === 'exchange' && state.withKey.value) {
        try {
          const count = await connectBybit(store.id, state.apiKey.value.trim(), state.apiSecret.value.trim(), {
            funding: t('subType.funding'),
            unified: t('subType.unified'),
            earn: t('subType.earn'),
          })
          notify.success('wallet.bybit.keySaved', { description: t('wallet.bybit.accountsReady', { count }) })
        } catch (err) {
          notify.error(err, 'wallet.bybit.keySaveFailed')
        }
      }
      return store
    },
    onSuccess: (store) => {
      for (const queryKey of [['stores'], ['store'], ['summary'], ['net-worth'], ['transactions'], ['history']]) {
        void queryClient.invalidateQueries({ queryKey })
      }
      state.open.value = false
      onCreated(store)
    },
    meta: {
      error: 'stores.createFailed',
      success: (data) => ({
        text: state.props.parent ? t('stores.subAccountCreated') : t('stores.created'),
        description: (data as Store).name,
      }),
    },
  })
}
