import { computed, type ComputedRef, nextTick, type Ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { settingsQuery, withoutSettings } from '@/shared/lib/settings-link'
import { useIsPhone } from '@/shared/lib/phone'

import { SETTINGS_LIST, type SettingsGroup, type SettingsTab } from './settings-shell.constants'
import { resolveSettingsTarget } from './settings-shell.resolve'

/**
 * The window lives in the URL: `?settings=<group>&section=<id>` opens it anywhere.
 * On a phone `?settings=list` is the list of groups and a group is a screen above
 * it, so the system Back goes group → list → closed with no extra code.
 */
export function useSettingsNav(groups: ComputedRef<SettingsGroup[]>, scroller: Ref<HTMLElement | null>) {
  const route = useRoute()
  const router = useRouter()
  const isPhone = useIsPhone()

  const open = computed({
    get: () => typeof route.query.settings === 'string',
    set: (value: boolean) => {
      if (!value) void router.replace({ query: withoutSettings(route.query) })
    },
  })
  const target = computed(() => resolveSettingsTarget(groups.value, route.query.settings, route.query.section))
  const activeGroup = computed(() => target.value.group)
  const activeTab = computed(() => target.value.tab)
  /** No group named: a phone shows the list of groups. */
  const isList = computed(() => !groups.value.some((group) => group.id === route.query.settings))

  /** The list entry is in the history under the group screen, so «‹» can go back to it. */
  let listBelow = false
  const query = (group: string, section?: string) => ({ query: settingsQuery(route.query, group, section) })

  watch(
    [open, isPhone, isList],
    ([isOpen, phone, list]) => {
      if (!isOpen) listBelow = false
      else if (phone && !list && !listBelow) {
        // A link straight to a group: put the list under it.
        listBelow = true
        const group = activeGroup.value.id
        const section = typeof route.query.section === 'string' ? route.query.section : undefined
        void router.replace(query(SETTINGS_LIST)).then(() => router.push(query(group, section)))
      }
    },
    { immediate: true },
  )

  function selectGroup(id: string) {
    if (isPhone.value && isList.value) {
      listBelow = true
      void router.push(query(id))
    } else void router.replace(query(id))
  }
  function selectTab(tab: SettingsTab) {
    void router.replace(query(activeGroup.value.id, tab.sections[0].id))
  }
  function backToList() {
    if (listBelow) router.back()
    else void router.replace(query(SETTINGS_LIST))
  }

  /* --- a tab shows its own sections; a link to one of several scrolls to it --- */
  watch(
    [open, isList, () => activeGroup.value.id, () => activeTab.value.id],
    async ([isOpen]) => {
      if (!isOpen) return
      await nextTick()
      const section = target.value.section
      // Scroll only the sections pane: scrollIntoView would also move the dialog and hide its header.
      const pane = scroller.value
      const el = section ? pane?.querySelector<HTMLElement>(`[data-section="${section}"]`) : null
      const top =
        pane && el ? el.getBoundingClientRect().top - pane.getBoundingClientRect().top + pane.scrollTop - 16 : 0
      pane?.scrollTo({ top: Math.max(0, top) })
    },
    { immediate: true },
  )

  return { open, isPhone, isList, activeGroup, activeTab, selectGroup, selectTab, backToList }
}
