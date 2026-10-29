import { computed } from 'vue'
import { createSharedComposable } from '@vueuse/core'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'

import { textOf } from '@/shared/lib/notify'

import { call } from './ipc'

/** The sidebar's «Pinned» block (`get_pinned`): every pin change writes it back. */
export const PINNED_KEY = ['pinned']

const sameTarget = (left: EntityRef, right: EntityRef) => left.kind === right.kind && left.id === right.id

/** The pins with their short values. */
export const usePinned = () => useQuery({ queryKey: PINNED_KEY, queryFn: () => call<PinnedView[]>('get_pinned') })

interface PinsChange {
  pins: Pinned[]
  /** The list before the change, for `Undo`. */
  previous: Pinned[]
  /** Which toast: an entity was pinned, unpinned, or a filter saved. */
  notice: 'pinned' | 'unpinned' | 'filterSaved'
}

/** Reads and changes the pinned list: one mutation, the whole list each time. */
function pinsState() {
  const queryClient = useQueryClient()
  const query = usePinned()
  const pins = computed(() => (query.data.value ?? []).map((view) => view.pin))
  const mutation = useMutation({
    mutationFn: ({ pins: next }: PinsChange) => call<PinnedView[]>('set_pinned', { pins: next }),
    onSuccess: (views) => queryClient.setQueryData(PINNED_KEY, views),
    meta: {
      error: 'pins.saveFailed',
      success: (_data: unknown, change: unknown) => textOf(`pins.${(change as PinsChange).notice}`),
      undo: async (_data: unknown, change: unknown) => {
        const views = await call<PinnedView[]>('set_pinned', { pins: (change as PinsChange).previous })
        queryClient.setQueryData(PINNED_KEY, views)
      },
    },
  })
  const save = (next: Pinned[], notice: PinsChange['notice']) =>
    mutation.mutate({ pins: next, previous: pins.value, notice })

  const isPinned = (target: EntityRef) =>
    pins.value.some((pin) => pin.type === 'entity' && sameTarget(pin.target, target))

  return {
    query,
    pins,
    isPinned,
    pending: mutation.isPending,
    toggle: (target: EntityRef) =>
      isPinned(target)
        ? save(
            pins.value.filter((pin) => !(pin.type === 'entity' && sameTarget(pin.target, target))),
            'unpinned',
          )
        : save([...pins.value, { type: 'entity', target }], 'pinned'),
    saveFilter: (name: string, query: string) =>
      save([...pins.value, { type: 'filter', id: '', name, query }], 'filterSaved'),
    remove: (index: number) =>
      save(
        pins.value.filter((_pin, at) => at !== index),
        'unpinned',
      ),
  }
}

/** One shared instance: every entity chip asks it whether its entity is pinned. */
export const usePins = createSharedComposable(pinsState)
