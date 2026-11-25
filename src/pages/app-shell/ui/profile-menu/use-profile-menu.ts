import { computed, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import {
  Bell,
  BellRing,
  Copy,
  EyeOff,
  GraduationCap,
  Lock,
  RefreshCw,
  Repeat,
  Settings,
  TriangleAlert,
  Workflow,
} from 'lucide-vue-next'
import type { Component } from 'vue'

import { useVaultDelete } from '@/features/vault-delete'
import { useVaultForget } from '@/features/vault-forget'
import { useSyncIndicator } from '@/features/vault-sync'
import { useUnreadNotices } from '@/widgets/notification-bell'
import { pluralForm } from '@/shared/lib/format'
import { notify } from '@/shared/lib/notify'
import { settingsQuery } from '@/shared/lib/settings-link'
import { useVaultStore } from '@/shared/store/vault'

import { type VaultActionId, vaultActionIds } from './vault-actions'

export interface ProfileEntry {
  id: string
  icon: Component
  label: string
  hint?: string
  destructive?: boolean
  /** A small number beside the label (unread notifications). */
  count?: number
  /** Colour of the icon, for a state (sync trouble). */
  iconClass?: string
  run: () => void
}

export interface ProfileMenuOptions {
  /** This screen has a tour, and how many steps it has. */
  tour: () => { available: boolean; steps: number }
  startTour: () => void
  allTours: () => void
}

/**
 * The profile menu's model: the vault actions of the open vault, then the status rows that left the
 * top bar (sync, notifications, tours), Automations, Reminders and Settings. Every entry goes where
 * it went before (spec D10): the same routes, the settings window, the lock.
 */
export function useProfileMenu(options: ProfileMenuOptions) {
  const { t, locale } = useI18n()
  const router = useRouter()
  const vault = useVaultStore()
  const { forget } = useVaultForget()
  const { saveCopy } = useVaultDelete()

  const open = ref(false)
  const noticesOpen = ref(false)
  const { unread } = useUnreadNotices()
  const sync = useSyncIndicator()
  /** Icon and colour of the sync row: busy, then unreliable clock, then connected devices. */
  function syncLook(): { icon: Component; iconClass?: string } {
    if (sync.busy.value) return { icon: RefreshCw, iconClass: 'animate-spin' }
    if (sync.unreliable.value) return { icon: TriangleAlert, iconClass: 'text-warning' }
    if (sync.connected.value) return { icon: RefreshCw, iconClass: 'text-success' }
    return { icon: RefreshCw }
  }
  /** The profile button's dot: something unread, or sync in trouble. */
  const attention = computed(() => unread.value > 0 || (sync.enabled.value && sync.unreliable.value))

  // The history is read when the menu opens: the row of the open vault gives forget and copy; the open vault is never deleted from here (the backend refuses it).
  watch(open, (isOpen) => {
    if (isOpen && !vault.sandbox) void vault.loadRecents().catch((err: unknown) => notify.error(err))
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
  const vaultActions = computed<Record<VaultActionId, ProfileEntry>>(() => ({
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
  }))

  const vaultEntries = computed<ProfileEntry[]>(() =>
    vault.sandbox ? [] : vaultActionIds(vault.encrypted, current.value).map((id) => vaultActions.value[id]),
  )
  const statusEntries = computed<ProfileEntry[]>(() => {
    const entries: ProfileEntry[] = []
    if (sync.enabled.value) {
      entries.push({
        id: 'sync',
        ...syncLook(),
        label: t('sync.title'),
        hint: sync.label.value,
        run: () => close(() => void router.push({ query: settingsQuery(router.currentRoute.value.query, 'sync') })),
      })
    }
    entries.push({
      id: 'notifications',
      icon: BellRing,
      label: t('notices.title'),
      count: unread.value || undefined,
      run: () => close(() => (noticesOpen.value = true)),
    })
    const tour = options.tour()
    if (tour.available) {
      const count = tour.steps
      entries.push({
        id: 'tour',
        icon: GraduationCap,
        label: t('tour.menuShow'),
        hint: t(`tour.menuShowHint.${pluralForm(count, locale.value)}`, { count }),
        run: () => close(options.startTour),
      })
    }
    entries.push({
      id: 'tours',
      icon: GraduationCap,
      label: t('tour.menuAll'),
      hint: t('tour.menuAllHint'),
      run: () => close(options.allTours),
    })
    return entries
  })
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

  return { open, noticesOpen, attention, current, vaultName, vaultEntries, statusEntries, appEntries }
}
