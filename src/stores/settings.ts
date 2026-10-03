import { defineStore } from 'pinia'

import { call } from '@/lib/ipc'

export const useSettingsStore = defineStore('settings', {
  state: () => ({
    settings: null as Settings | null,
  }),
  actions: {
    async load() {
      this.settings = await call<Settings>('get_settings')
    },
    async update(patch: Partial<Settings>) {
      this.settings = await call<Settings>('update_settings', { input: patchToInput(patch) })
    },
  },
})

function patchToInput(patch: Partial<Settings>) {
  return {
    base_currency: patch.base_currency ?? null,
    theme: patch.theme ?? null,
    locale: patch.locale ?? null,
    display_decimals: patch.display_decimals ?? null,
  }
}