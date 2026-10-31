import { describe, expect, it } from 'vitest'

import { assign, median, parseShare, plan, takeShare, type Unit, weigh } from './shard'

// A list like the real one: a few heavy units, many small, some equal weights (ties by name).
const units: Unit[] = [
  ...Array.from({ length: 40 }, (_unused, at): Unit => [`dark-1280 #${at + 1}`, 3 + (at % 7)]),
  ...Array.from({ length: 12 }, (_unused, at): Unit => [`overlays-390 #${at + 1}`, 20 + at]),
  ['Back closes layers', 3],
  ['dialog opens in place', 3],
  ['no listener leaks', 3],
  ['huge', 90],
]
const names = units.map(([name]) => name)

describe('shard', () => {
  it('every unit lands in exactly one share', () => {
    for (const total of [1, 2, 3, 6, 7]) {
      const seen = Array.from({ length: total }, (_unused, at) =>
        takeShare(units, [at + 1, total]).map(([name]) => name),
      )
      expect(seen.flat().sort()).toEqual([...names].sort())
    }
  })

  it('is deterministic whatever the order of the list', () => {
    const shuffled = [...units].reverse()
    expect(assign(units, 6)).toEqual(assign(shuffled, 6))
    expect(assign(units, 6)).toEqual(assign(units, 6))
  })

  it('shard x jobs covers every unit exactly once', () => {
    const seen: string[] = []
    for (let ci = 1; ci <= 6; ci++) {
      for (let job = 1; job <= 3; job++) seen.push(...plan(units, [ci, 6], [job, 3]))
    }
    expect(seen.sort()).toEqual([...names].sort())
    expect(plan(units, [1, 1], [1, 1])).toEqual(names)
  })

  it('keeps the order of the list inside a share', () => {
    const mine = plan(units, [2, 6], [1, 2])
    expect(mine).toEqual(names.filter((name) => mine.includes(name)))
  })

  it('balances: the heaviest share is at most the average plus the biggest unit', () => {
    const biggest = Math.max(...units.map(([, weight]) => weight))
    for (const total of [2, 3, 4, 6]) {
      const loads = Array.from({ length: total }, (_unused, at) =>
        takeShare(units, [at + 1, total]).reduce((sum, [, weight]) => sum + weight, 0),
      )
      const average = units.reduce((sum, [, weight]) => sum + weight, 0) / total
      expect(Math.max(...loads)).toBeLessThanOrEqual(average + biggest)
    }
  })

  it('refuses a unit listed twice', () => {
    expect(() =>
      assign(
        [
          ['a', 1],
          ['a', 2],
        ],
        2,
      ),
    ).toThrow(/twice/)
  })

  it('a unit without a weight weighs the median, a stray key is ignored', () => {
    expect(median([1, 3, 10])).toBe(3)
    expect(median([1, 3])).toBe(2)
    expect(median([])).toBe(1)
    expect(weigh(['a', 'b', 'c'], { a: 5, b: 1, c: 9, gone: 100 }).map(([, weight]) => weight)).toEqual([5, 1, 9])
    expect(weigh(['a', 'new'], { a: 5, b: 1, c: 9 })).toEqual([
      ['a', 5],
      ['new', 5],
    ])
    expect(weigh(['x'], {})).toEqual([['x', 1]])
  })

  it('parses K/N', () => {
    expect(parseShare(undefined, 'x')).toEqual([1, 1])
    expect(parseShare('2/6', 'x')).toEqual([2, 6])
    for (const bad of ['0/3', '4/3', '1', 'a/b', '1/0']) expect(() => parseShare(bad, 'x')).toThrow()
  })
})
