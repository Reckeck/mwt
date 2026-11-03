import { type MaybeRefOrGetter, ref, toValue, watch } from 'vue'

import { sameData, threeWay } from './three-way'

export interface EditBaseOptions<Entity, Model> {
  /** Whether the form is open (a form that exists only while open passes `true`). */
  open: MaybeRefOrGetter<boolean>
  /** The entity the form loads its fields from (the prop); snapshotted when the form opens. */
  entity: () => Entity | null | undefined
  /** The live entity (a query that `vault-synced` refreshes); `undefined` while it is not known. */
  live: () => Entity | null | undefined
  /** The form's fields of an entity; defaults to the entity itself. */
  model?: (entity: Entity) => Model
}

const clone = <Value>(value: Value): Value => (value === undefined ? value : JSON.parse(JSON.stringify(value)))

/**
 * Edit forms and sync (B3, catalog 97): the form keeps the entity as it opened (`base`, sent
 * with the update so the core applies only the user's own changes) and notices when the live
 * record stops matching it (`changedElsewhere`, drives `SyncChangedBanner`).
 * `refresh` merges the other device's values into the untouched fields and keeps the user's.
 */
export function useEditBase<Entity, Model = Entity>(options: EditBaseOptions<Entity, Model>) {
  const base = ref<Entity>()
  const changedElsewhere = ref(false)
  const modelOf = options.model ?? ((entity: Entity) => entity as unknown as Model)

  function check() {
    const live = options.live()
    changedElsewhere.value = base.value !== undefined && !!live && !sameData(live, base.value)
  }

  watch(
    () => toValue(options.open),
    (isOpen) => {
      base.value = isOpen ? clone(options.entity() ?? undefined) : undefined
      check()
    },
    { immediate: true, flush: 'sync' },
  )

  // A prop that is older than the live record shows the banner at once.
  watch(() => options.live(), check, { deep: true })

  /** `current` reads the form's fields as a plain object, `load` writes a merged model back. */
  function refresh(current: () => Model, load: (model: Model) => void) {
    const live = options.live()
    if (base.value === undefined || !live) return
    load(threeWay(modelOf(base.value), clone(current()), modelOf(clone(live))))
    base.value = clone(live)
    changedElsewhere.value = false
  }

  return { base, changedElsewhere, refresh }
}
