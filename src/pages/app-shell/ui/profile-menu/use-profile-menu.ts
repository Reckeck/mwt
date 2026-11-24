import { computed, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { Bell, Copy, EyeOff, Lock, Repeat, Settings, Workflow } from 'lucide-vue-next'
import type { Component } from 'vue'

import { useVaultDelete } from '@/features/vault-delete'
import { useVaultForget } from '@/features/vault-forget'
import { settingsQuery } from '@/shared/lib/settings-link'
import { useVaultStore } from '@/shared/store/vault'

import { type VaultActionId, vaultActionIds } from './vault-actions'

export interface ProfileEntry {
  id: string
  icon: Component
  label: string
  hint?: string
  destructive?: boolean
  run: () => void
}

/**
 * The profile menu's model: the vault actions of the open vault, then Automations, Reminders and
 * Settings. Every entry goes where it went before (spec D10): the same routes, the settings
 * window, the lock.
 */
export function useProfileMenu() {
  const { t } = useI18n()
  const router = useRouter()
  const vault = useVaultStore()
  const { forget } = useVaultForget()
  const { saveCopy } = useVaultDelete()

  const open = ref(false)

  // The history is read when the menu opens: the row of the open vault gives forget and copy; the open vault is never deleted from here (the backend refuses it).
  watch(open, (isOpen) => {
    if (isOpen && !vault.sandbox) void vault.loadRecents().catch(() => {})
  })

  const current = computed(() => vault.recents.find((recent: RecentVault) => recent.path === vault.status?.path))
  const vaultName = computed(() => {
    if (vault.sandbox) return t('sandbox.sidebar')
    return current.value?.display_name || (vault.status?.path ?? '').split(/[\\/]/).pop() || t('app.title')
  })

  function close(run: () => void) {
    open.value = false
    run()
  }
  const vaultActions: Record<VaultActionId, ProfileEntry> = {
    lock: {
      id: 'lock',
      icon: Lock,
      label: t('auth.lock'),
      hint: t('nav.moreItem.lock'),
      run: () => close(() => void vault.lock()),
    },
    switch: {
      id: 'switch',
      icon: Repeat,
      label: t('profile.switch'),
      hint: t('profile.switchHint'),
      run: () => close(() => void vault.lock()),
    },
    saveCopy: {
      id: 'saveCopy',
      icon: Copy,
      label: t('vaultDelete.saveCopy'),
      run: () => close(() => current.value && saveCopy.mutate(current.value.path)),
    },
    forget: {
      id: 'forget',
      icon: EyeOff,
      label: t('profile.forget'),
      hint: t('profile.forgetHint'),
      run: () => close(() => current.value && forget.mutate({ path: current.value.path, name: vaultName.value })),
    },
  }

  const vaultEntries = computed<ProfileEntry[]>(() =>
    vault.sandbox ? [] : vaultActionIds(vault.encrypted, current.value).map((id) => vaultActions[id]),
  )
  const appEntries = computed<ProfileEntry[]>(() => [
    {
      id: 'automations',
      icon: Workflow,
      label: t('nav.automations'),
      hint: t('profile.automationsHint'),
      run: () => close(() => void router.push({ name: 'automations' })),
    },
    {
      id: 'reminders',
      icon: Bell,
      label: t('planning.tab.reminders'),
      run: () => close(() => void router.push({ name: 'automations-reminders' })),
    },
    {
      id: 'settings',
      icon: Settings,
      label: t('settingsShell.title'),
      hint: t('nav.moreItem.settings'),
      run: () => close(() => void router.push({ query: settingsQuery(router.currentRoute.value.query, 'list') })),
    },
  ])

  return { open, current, vaultName, vaultEntries, appEntries }
}
