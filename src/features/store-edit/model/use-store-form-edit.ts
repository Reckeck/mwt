import { watch } from 'vue'

import { useEditBase } from '@/shared/lib/form'

import { updateInputOf, useStoreUpdate } from './use-store-update'

/**
 * B3: a wallet settings form that stays on screen. The page's wallet is the live record (the
 * overview is re-read after `vault-synced`), the form keeps the wallet as the editing began
 * (`base`, sent with the update) and shows `SyncChangedBanner` when they differ. It does not
 * refill from the prop, so a sync never overwrites what the user is typing; `Refresh` does the
 * merge. Editing begins again for another wallet and after the form's own save.
 */
export function useStoreFormEdit<Model>(options: {
  store: () => Store
  /** The form's fields of a wallet. */
  model: (store: Store) => Model
  /** Reads the form's fields as they are typed. */
  current: () => Model
  /** Writes fields into the form. */
  load: (model: Model) => void
}) {
  const edit = useEditBase({ open: true, entity: options.store, live: options.store, model: options.model })

  watch(
    () => options.store().id,
    () => {
      options.load(options.model(options.store()))
      edit.rebase()
    },
    { immediate: true },
  )

  const update = useStoreUpdate()
  /** Every field of the update comes from the snapshot, the form patches only its own part. */
  function save(patch: Partial<UpdateStoreInput>) {
    update.mutate(updateInputOf(edit.base.value ?? options.store(), { ...patch, base: edit.base.value }), {
      onSuccess: (saved) => edit.rebase(saved),
    })
  }

  return {
    save,
    isPending: update.isPending,
    changedElsewhere: edit.changedElsewhere,
    refreshFromOtherDevice: () => edit.refresh(options.current, options.load),
  }
}
