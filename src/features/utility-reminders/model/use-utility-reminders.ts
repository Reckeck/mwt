import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

/** Every field `update_store` takes, filled from the store so only the pay day changes. */
function updateInputOf(store: Store, patch: Partial<UpdateStoreInput>): UpdateStoreInput {
  return {
    id: store.id,
    name: store.name,
    color: store.color,
    note: store.note,
    purpose: null,
    binding: null,
    liability: store.liability,
    contact_id: store.contact_id,
    custody: null,
    property: null,
    secured_by: store.secured_by,
    provider: null,
    ...patch,
  }
}

const STORE_STALE_KEYS = [
  ['stores'],
  ['store'],
  ['summary'],
  ['net-worth'],
  ['budget'],
  ['daily-budget'],
  ['emergency-fund'],
]

/**
 * Switching the property's automatic utility reminders on, moving their day
 * or off: they come from its utilities pay day, which the backend turns into
 * «Pay the utilities» each month and «Send the readings» three days earlier.
 */
export function useUtilityReminders() {
  const queryClient = useQueryClient()
  const save = (store: Store, day: number | null) =>
    call<Store>('update_store', {
      input: updateInputOf(store, {
        property: store.property ? { ...store.property, utilities_pay_day: day ?? undefined } : null,
      }),
    })
  const refresh = () => {
    for (const queryKey of [...STORE_STALE_KEYS, ['reminders'], ['upcoming']]) {
      void queryClient.invalidateQueries({ queryKey })
    }
  }

  const setDay = useMutation({
    mutationFn: ({ store, day }: { store: Store; day: number }) => save(store, day),
    onSuccess: refresh,
    meta: { success: 'utilityReminders.saved', error: 'stores.saveFailed' },
  })
  const turnOff = useMutation({
    mutationFn: (store: Store) => save(store, null),
    onSuccess: refresh,
    meta: { success: 'utilityReminders.turnedOff', error: 'stores.saveFailed' },
  })
  return { setDay, turnOff }
}
