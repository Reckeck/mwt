import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { PLANNING_KEYS } from '@/shared/lib/planning'

import { utilityInputOf } from '../lib/utility-fields'
import type { UtilityDialogProps } from '../utility-edit.types'
import type { UtilityForm } from './use-utility-form'

/** Creates or updates the item; an edited one keeps whether it is on. */
export function useSaveUtility(props: UtilityDialogProps, state: UtilityForm, done: () => void) {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: () => {
      const input = state.currentInput()
      return props.item
        ? call<UtilityItem>('update_utility_item', {
            input: {
              id: props.item.id,
              item: input,
              base: state.editBase.value && utilityInputOf(state.editBase.value),
            },
          })
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
