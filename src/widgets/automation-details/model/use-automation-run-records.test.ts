import { describe, expect, it } from 'vitest'

import { PLANNING_KEYS } from '@/shared/lib/planning'

import { AUTOMATION_RUNS_KEY } from './use-automation-run-records'

describe('run records cache key', () => {
  it('is invalidated by the planning refresh a retry, skip or confirm runs', () => {
    expect(PLANNING_KEYS).toContainEqual([...AUTOMATION_RUNS_KEY])
  })
})
