import { type Ref, computed, ref, watch } from 'vue'
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { usePins } from '@/shared/api/pins'
import { required, useFormValidation } from '@/shared/lib/form'
import { textOf } from '@/shared/lib/notify'

import { STORE_STALE_KEYS, updateInputOf, useStoreUpdate } from './use-store-update'

/** Only everyday money is in the day's budget: the switch and the purpose are one setting. */
const BUDGET_PURPOSE: Purpose = 'everyday'

/**
 * `Settings · <wallet>` (Wallets-3 · 19): the name, the budget and favourite
 * switches and the purpose, saved together; the archive is its own action.
 */
export function useStoreSettings(store: () => Store, open: Ref<boolean>, onDone: () => void) {
  const queryClient = useQueryClient()
  const pins = usePins()
  const target = computed<EntityRef>(() => ({ kind: 'store', id: store().id }))

  const name = ref('')
  const purpose = ref<Purpose>('everyday')
  const favorite = ref(false)
  /** What the budget switch goes back to when it is turned off again. */
  const offPurpose = ref<Purpose>('investment')

  /** A loan's purpose is fixed: it is never everyday money. */
  const purposeEditable = computed(() => store().kind !== 'liability')

  watch(open, (isOpen) => {
    if (!isOpen) return
    name.value = store().name
    purpose.value = store().purpose
    offPurpose.value = store().purpose === BUDGET_PURPOSE ? 'investment' : store().purpose
    favorite.value = pins.isPinned(target.value)
  })

  const inBudget = computed({
    get: () => purpose.value === BUDGET_PURPOSE,
    set: (on: boolean) => {
      if (!on) purpose.value = offPurpose.value
      else {
        if (purpose.value !== BUDGET_PURPOSE) offPurpose.value = purpose.value
        purpose.value = BUDGET_PURPOSE
      }
    },
  })
  watch(purpose, (next) => {
    if (next !== BUDGET_PURPOSE) offPurpose.value = next
  })

  const form = useFormValidation(() => ({ name: required(name.value) }), { resetOn: [open] })

  const update = useStoreUpdate()
  function save() {
    update.mutate(
      updateInputOf(store(), {
        name: name.value.trim(),
        purpose: purposeEditable.value ? purpose.value : null,
      }),
      {
        onSuccess: () => {
          if (favorite.value !== pins.isPinned(target.value)) pins.toggle(target.value)
          onDone()
        },
      },
    )
  }

  const archive = useMutation({
    mutationFn: (archived: boolean) => call('archive_store', { id: store().id, archived }),
    onSuccess: () => {
      for (const queryKey of STORE_STALE_KEYS) void queryClient.invalidateQueries({ queryKey })
      onDone()
    },
    meta: {
      error: 'wallet.archiveFailed',
      success: (_data: unknown, archived: unknown) => textOf(archived ? 'wallet.archived' : 'wallet.unarchived'),
    },
  })

  return { name, purpose, favorite, inBudget, purposeEditable, form, save, pending: update.isPending, archive }
}
