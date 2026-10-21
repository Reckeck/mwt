import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { ASSET_CLASSES_KEY, TOPUP_ADVICE_KEY } from '@/shared/api/queries'
import { textOf } from '@/shared/lib/notify'
import { STORE_TREE_QUERY_KEY } from '@/shared/lib/store-tree'

export interface ClassesChange {
  classes: AssetClass[]
  /** What «Отменить» puts back; an empty list is the preset again. */
  previous: AssetClass[]
}

export interface AssignChange {
  target: AssetClassTarget
  classId: string | null
  previous: string | null
}

function useRefresh() {
  const queryClient = useQueryClient()
  return () => {
    for (const queryKey of [ASSET_CLASSES_KEY, TOPUP_ADVICE_KEY, ['net-worth'], STORE_TREE_QUERY_KEY, ['store']])
      void queryClient.invalidateQueries({ queryKey })
  }
}

/** Saves the whole list of baskets with their shares. */
export function useClassesSet() {
  const refresh = useRefresh()
  return useMutation({
    mutationFn: ({ classes }: ClassesChange) => call<AssetClassesView>('asset_classes_set', { classes }),
    onSuccess: refresh,
    meta: {
      error: 'assetClass.saveFailed',
      success: () => textOf('assetClass.saved'),
      undo: (_data, change) =>
        call<AssetClassesView>('asset_classes_set', { classes: (change as ClassesChange).previous }),
    },
  })
}

/** Puts an asset or a wallet into a basket (`null` takes it out). */
export function useAssetClassAssign() {
  const refresh = useRefresh()
  return useMutation({
    mutationFn: ({ target, classId }: AssignChange) =>
      call<AssetClassesView>('asset_class_assign', { target, classId }),
    onSuccess: refresh,
    meta: {
      error: 'assetClass.assignFailed',
      success: () => textOf('assetClass.assigned'),
      undo: (_data, change) => {
        const { target, previous } = change as AssignChange
        return call<AssetClassesView>('asset_class_assign', { target, classId: previous })
      },
    },
  })
}
