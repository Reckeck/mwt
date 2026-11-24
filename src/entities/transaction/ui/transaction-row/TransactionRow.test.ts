import { mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'

vi.mock('@/shared/lib/phone', async () => {
  const { ref } = await import('vue')
  return { useIsPhone: () => ref(false) }
})
vi.mock('@/shared/lib/tx-context', () => ({
  useTxContext: () => ({
    storeName: (id: string) => id,
    rootOf: (id: string) => id,
    contactName: (id: string) => id,
    sideLabel: () => '',
    categoryName: (key: string) => key,
    attachmentCount: () => 0,
  }),
}))

import TransactionRow from './TransactionRow.vue'

const payment = (comment: string) =>
  ({
    payment: {
      meta: {
        id: 'tx-1',
        timestamp: '2026-11-01T10:00:00Z',
        comment,
        categories: [],
        flow_id: null,
        contact_id: null,
        origin: 'manual',
      },
      store_id: 'cash',
      asset: { symbol: 'RUB', kind: 'fiat' },
      amount: '100',
      to: null,
      details: { network: null, tx_hash: null },
      fee: null,
      price: null,
    },
  }) as unknown as Transaction

describe('TransactionRow', () => {
  it('shows the notes in the second line on desktop', () => {
    const wrapper = mount(TransactionRow, {
      props: { tx: payment('paid in cash') },
      global: { stubs: { Money: true } },
    })
    expect(wrapper.find('[data-tx-notes]').text()).toContain('paid in cash')
  })
})
