import { reactive } from 'vue'
import { describe, expect, it } from 'vitest'

import { useTxCalcNote } from './use-tx-calc-note'
import type { TxFormState } from './use-tx-form-state'

function setup(comment = '') {
  const form = reactive({ comment })
  const { noteExpression } = useTxCalcNote({ form } as unknown as TxFormState)
  return { form, noteExpression }
}

describe('useTxCalcNote', () => {
  it('appends a line below the hand-written comment', () => {
    const { form, noteExpression } = setup('lunch')
    noteExpression('tx-amount', { expr: '1200+350*2', result: '1900' })
    expect(form.comment).toBe('lunch\n1200+350*2 = 1900')
  })

  it('replaces the line of the same field, leaves other fields alone', () => {
    const { form, noteExpression } = setup('lunch')
    noteExpression('tx-amount', { expr: '1+2', result: '3' })
    noteExpression('tx-fee', { expr: '10%', result: '0.1' })
    noteExpression('tx-amount', { expr: '2+2', result: '4' })
    expect(form.comment).toBe('lunch\n2+2 = 4\n10% = 0.1')
  })

  it('keeps a comment the user edited around the line', () => {
    const { form, noteExpression } = setup()
    noteExpression('tx-amount', { expr: '1+2', result: '3' })
    form.comment = `taxi\n${form.comment}\nthanks`
    noteExpression('tx-amount', { expr: '5+5', result: '10' })
    expect(form.comment).toBe('taxi\n5+5 = 10\nthanks')
  })
})
