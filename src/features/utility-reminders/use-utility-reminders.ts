import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { STORE_STALE_KEYS, updateInputOf } from '@/features/store-edit'
import { call } from '@/shared/api/ipc'

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
