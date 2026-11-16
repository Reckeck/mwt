import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import NetworkEditor from './NetworkEditor.vue'

const endpoint = (value: string, presets: string[] = [value]): EndpointView => ({
  value,
  custom: false,
  presets,
  host: value.replace('https://', ''),
})

const network: NetworkView = {
  chain: 'base',
  label: 'Base',
  chain_id: 8453,
  rpc: endpoint('https://mainnet.base.org', ['https://mainnet.base.org', 'https://base-rpc.publicnode.com']),
  blockscout: endpoint('https://base.blockscout.com'),
  explorer: endpoint('https://basescan.org'),
  blockscout_key_set: false,
}

function mountEditor(editable = true) {
  return mount(NetworkEditor, { props: { network, editable, pending: false, testing: false } })
}

describe('NetworkEditor', () => {
  it('keeps the save button active and shows a wrong address under its field', async () => {
    const wrapper = mountEditor()
    const save = wrapper.find('button[type="submit"]')
    expect(save.attributes('disabled')).toBeUndefined()

    await wrapper.find('#crypto-network-rpc').setValue('http://node.example.org')
    expect(save.attributes('disabled')).toBeUndefined()
    await wrapper.find('form').trigger('submit')

    expect(wrapper.emitted('save')).toBeUndefined()
    expect(wrapper.find('#crypto-network-rpc').attributes('aria-invalid')).toBe('true')
    expect(wrapper.text()).toContain('https')
  })

  it('saves a valid address, with the key left alone unless one was typed', async () => {
    const wrapper = mountEditor()
    await wrapper.find('#crypto-network-rpc').setValue('http://localhost:8545')
    await wrapper.find('form').trigger('submit')
    const [input] = wrapper.emitted('save')?.[0] ?? []
    expect(input).toMatchObject({ chain: 'base', rpc: 'http://localhost:8545', blockscout: '', explorer: '' })
    expect((input as SetNetworkSettingsInput).blockscout_key).toBeUndefined()

    await wrapper.find('#crypto-network-key').setValue('proapi_abc')
    await wrapper.find('form').trigger('submit')
    const second = (wrapper.emitted('save') ?? [])[1] ?? []
    expect((second[0] as SetNetworkSettingsInput).blockscout_key).toBe('proapi_abc')
  })

  it('masks the key field and never fills it from the stored key', () => {
    const wrapper = mount(NetworkEditor, {
      props: { network: { ...network, blockscout_key_set: true }, editable: true, pending: false, testing: false },
    })
    const field = wrapper.find('#crypto-network-key')
    expect(field.attributes('type')).toBe('password')
    expect((field.element as HTMLInputElement).value).toBe('')
  })

  it('does not test a node when the typed address is wrong', async () => {
    const wrapper = mountEditor()
    await wrapper.find('#crypto-network-rpc').setValue('ftp://node.example.org')
    const testNode = wrapper.findAll('button').find((button) => button.text() === 'Проверить узел')
    expect(testNode).toBeDefined()
    await testNode?.trigger('click')
    expect(wrapper.emitted('test')).toBeUndefined()
  })
})
