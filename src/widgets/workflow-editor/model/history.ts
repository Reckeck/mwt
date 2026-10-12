import { ref } from 'vue'

/** Most steps back «Отменить» keeps. */
const LIMIT = 100

/** Undo and redo over snapshots of the graph (its JSON). */
export function useHistory(apply: (snapshot: string) => void) {
  const past = ref<string[]>([])
  const future = ref<string[]>([])
  let current = ''

  /** Remember a new state; the same one twice is one step. */
  function record(snapshot: string) {
    if (snapshot === current) return
    if (current) past.value = [...past.value, current].slice(-LIMIT)
    future.value = []
    current = snapshot
  }

  function reset(snapshot: string) {
    past.value = []
    future.value = []
    current = snapshot
  }

  function undo() {
    const previous = past.value.at(-1)
    if (previous === undefined) return
    past.value = past.value.slice(0, -1)
    future.value = [current, ...future.value]
    current = previous
    apply(previous)
  }

  function redo() {
    const next = future.value[0]
    if (next === undefined) return
    future.value = future.value.slice(1)
    past.value = [...past.value, current]
    current = next
    apply(next)
  }

  return { record, reset, undo, redo, canUndo: () => past.value.length > 0, canRedo: () => future.value.length > 0 }
}
