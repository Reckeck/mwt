import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import { i18n } from '@/shared/config/i18n'
import en from '@/shared/config/i18n/locales/en.json'
import ru from '@/shared/config/i18n/locales/ru.json'

import StoreChangeCorrections from './StoreChangeCorrections.vue'
import { lineKey } from './store-change.line-key'

const shift: CorrectionShift = {
  tx_id: 'tx-1',
  timestamp: '2025-03-01T00:00:00Z',
  from_store_id: 'card-1',
  from_store_name: 'Card 1',
  asset: { symbol: 'USD', name: 'US Dollar', coingecko_id: null, chain: null, contract: null, quote: 'usd' },
  after_old: '80',
  after_new: '1080',
  other_balance: '1000',
}

const Money = { props: ['amount'], template: '<span class="money">{{ amount }}</span>' }

function render(corrections: CorrectionShift[]) {
  return mount(StoreChangeCorrections, {
    props: { corrections, source: { id: 'card-1', name: 'Card 1' }, targetName: 'Card 2' },
    global: { stubs: { Money } },
  })
}

describe('StoreChangeCorrections', () => {
  it('lists each shifted correction with wallet, date and the three amounts', () => {
    const wrapper = render([shift])
    const line = wrapper.find('li').text()
    expect(line).toContain('Card 1')
    expect(wrapper.findAll('.money').map((node) => node.text())).toEqual(['80', '1080', '1000'])
  })

  it('keys the shifted lines of one tx apart by asset and contract', () => {
    const eur = { ...shift.asset, symbol: 'EUR' }
    const usdOnChain = { ...shift.asset, contract: '0xabc' }
    const shifts = [shift, { ...shift, asset: eur }, { ...shift, asset: usdOnChain }]
    expect(new Set(shifts.map(lineKey)).size).toBe(shifts.length)
  })

  it('names the other wallet: the target for a source correction, the source otherwise', () => {
    const wrapper = render([shift, { ...shift, tx_id: 'tx-2', from_store_id: 'card-2', from_store_name: 'Card 2' }])
    const lines = wrapper.findAll('li').map((node) => node.text())
    expect(lines[0]).toContain('Card 2')
    expect(lines[1]).toContain('Card 1')
  })

  it('renders nothing when there are no corrections', () => {
    expect(render([]).find('[data-testid="store-change-corrections"]').exists()).toBe(false)
  })

  it('has the same storeChange correction keys in ru and en', () => {
    for (const key of ['storeChange.corrections', 'storeChange.correctionLine']) {
      expect(ru).toHaveProperty([key])
      expect(en).toHaveProperty([key])
    }
    expect(i18n.global.locale.value).toBe('ru')
  })
})
