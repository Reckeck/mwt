import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { PLANNING_KEYS } from '@/shared/lib/planning'

import type { UtilityDialogProps } from '../utility-edit.types'
import type { UtilityForm } from './use-utility-form'

/** Creates or updates the item; an edited one keeps whether it is on. */
export function useSaveUtility(props: UtilityDialogProps, state: UtilityForm, done: () => void) {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: () => {
      const input: UtilityInput = {
        name: state.name.value.trim(),
        active: props.item?.active ?? true,
        kind: state.utilityKind.value,
        currency: state.currency.value,
        property_id: state.propertyId.value,
        store_id: state.storeId.value || null,
        category: state.category.value,
      }
      return props.item
        ? call<UtilityItem>('update_utility_item', { input: { id: props.item.id, item: input } })
        : call<UtilityItem>('create_utility_item', { input })
    },
    onSuccess: () => {
      for (const queryKey of [['utilities'], ['utility-trend'], ['utility-bill'], ...PLANNING_KEYS])
        void queryClient.invalidateQueries({ queryKey })
      done()
    },
    meta: {
      error: 'utilities.saveFailed',
      success: () => (props.item ? t('utilities.saved') : t('utilities.created')),
    },
  })
}
