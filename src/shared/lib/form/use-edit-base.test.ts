import { describe, expect, it } from 'vitest'
import { nextTick, ref } from 'vue'

import { sameData, threeWay } from './three-way'
import { useEditBase } from './use-edit-base'

describe('threeWay', () => {
  it('takes the fresh value of an untouched field', () => {
    expect(threeWay({ name: 'a', note: 'x' }, { name: 'a', note: 'x' }, { name: 'b', note: 'x' })).toEqual({
      name: 'b',
      note: 'x',
    })
  })
  it('keeps the edit of a field changed on both sides', () => {
    expect(threeWay({ name: 'a' }, { name: 'mine' }, { name: 'theirs' })).toEqual({ name: 'mine' })
  })
  it('keeps the edit when only the user changed it', () => {
    expect(threeWay({ name: 'a', note: 'x' }, { name: 'mine', note: 'x' }, { name: 'a', note: 'y' })).toEqual({
      name: 'mine',
      note: 'y',
    })
  })
  it('recurses into objects and takes arrays whole', () => {
    const base = { scope: { ids: ['a'], deep: true }, label: 'l' }
    const current = { scope: { ids: ['a', 'b'], deep: true }, label: 'l' }
    const fresh = { scope: { ids: ['c'], deep: false }, label: 'l2' }
    expect(threeWay(base, current, fresh)).toEqual({ scope: { ids: ['a', 'b'], deep: false }, label: 'l2' })
  })
  it('compares data regardless of key order', () => {
    expect(sameData({ a: 1, b: [1, { c: 2 }] }, { b: [1, { c: 2 }], a: 1 })).toBe(true)
    expect(sameData({ a: 1 }, { a: 2 })).toBe(false)
  })
})

describe('useEditBase', () => {
  const setup = () => {
    const open = ref(false)
    const prop = ref({ name: 'a', note: 'x' })
    const live = ref<{ name: string; note: string } | undefined>(undefined)
    const edit = useEditBase({ open, entity: () => prop.value, live: () => live.value })
    return { open, prop, live, edit }
  }

  it('snapshots on open and starts clean', () => {
    const { open, prop, edit } = setup()
    expect(edit.base.value).toBeUndefined()
    open.value = true
    prop.value = { name: 'zzz', note: 'x' }
    expect(edit.base.value).toEqual({ name: 'a', note: 'x' })
    expect(edit.changedElsewhere.value).toBe(false)
  })

  it('flags a live change while open, not an identical live copy', async () => {
    const { open, live, edit } = setup()
    open.value = true
    live.value = { name: 'a', note: 'x' }
    await nextTick()
    expect(edit.changedElsewhere.value).toBe(false)
    live.value = { name: 'b', note: 'x' }
    await nextTick()
    expect(edit.changedElsewhere.value).toBe(true)
  })

  it('ignores a live change outside the form model', async () => {
    const open = ref(true)
    const live = ref({ name: 'a', note: 'x' })
    const edit = useEditBase({
      open,
      entity: () => ({ name: 'a', note: 'x' }),
      live: () => live.value,
      model: (entity: { name: string; note: string }) => ({ name: entity.name }),
    })
    live.value = { name: 'a', note: 'changed elsewhere' }
    await nextTick()
    expect(edit.changedElsewhere.value).toBe(false)
    live.value = { name: 'b', note: 'changed elsewhere' }
    await nextTick()
    expect(edit.changedElsewhere.value).toBe(true)
  })

  it('refresh merges, moves the base and clears the flag', async () => {
    const { open, live, edit } = setup()
    open.value = true
    live.value = { name: 'b', note: 'y' }
    await nextTick()
    let loaded: unknown
    edit.refresh(
      () => ({ name: 'a', note: 'mine' }),
      (model) => (loaded = model),
    )
    expect(loaded).toEqual({ name: 'b', note: 'mine' })
    expect(edit.base.value).toEqual({ name: 'b', note: 'y' })
    expect(edit.changedElsewhere.value).toBe(false)
  })

  it('rebase takes a new snapshot and clears the flag', async () => {
    const { open, live, edit } = setup()
    open.value = true
    live.value = { name: 'b', note: 'x' }
    await nextTick()
    expect(edit.changedElsewhere.value).toBe(true)
    edit.rebase({ name: 'b', note: 'x' })
    expect(edit.base.value).toEqual({ name: 'b', note: 'x' })
    expect(edit.changedElsewhere.value).toBe(false)
  })

  it('resets on close', async () => {
    const { open, live, edit } = setup()
    open.value = true
    live.value = { name: 'b', note: 'x' }
    await nextTick()
    open.value = false
    expect(edit.base.value).toBeUndefined()
    expect(edit.changedElsewhere.value).toBe(false)
  })
})
