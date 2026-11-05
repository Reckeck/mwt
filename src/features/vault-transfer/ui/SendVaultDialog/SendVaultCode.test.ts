import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import SendVaultCode from './SendVaultCode.vue'

const view = { phase: 'code', device_name: 'Phone', wrong_code: false, attempts_left: 3 } as unknown as TransferSendView

describe('SendVaultCode', () => {
  it('shows the error of the code command under the field', () => {
    const wrapper = mount(SendVaultCode, { props: { view, failure: 'SYNC_BAD_CODE: nope' } })
    expect(wrapper.text()).toContain('Код не подошёл')
  })

  it('shows nothing extra without a failure', () => {
    const wrapper = mount(SendVaultCode, { props: { view } })
    expect(wrapper.text()).not.toContain('Код не подошёл')
  })

  it('asks the dialog to clear the error when the user types', async () => {
    const wrapper = mount(SendVaultCode, { props: { view, failure: 'NOT_FOUND: x' } })
    await wrapper.find('input').setValue('1')
    expect(wrapper.emitted('edit')).toBeTruthy()
  })
})
