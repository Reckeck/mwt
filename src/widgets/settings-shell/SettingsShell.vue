<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChevronLeft, X } from 'lucide-vue-next'
import { DialogClose, DialogContent, DialogDescription, DialogOverlay, DialogPortal, DialogRoot, DialogTitle } from 'reka-ui'

import { Button } from '@/shared/ui/button'
import { useVaultStore } from '@/shared/store/vault'
import { useAutolockActivity } from '@/features/vault-autolock'
import { Tabs } from '@/shared/ui/tabs'
import { usePlatform } from '@/shared/lib/platform'
import { useCoarsePointer } from '@/shared/lib/phone'
import { useTour } from '@/shared/lib/tour'
import { keepOpenOnToast } from '@/shared/lib/toast-outside'
import { cn } from '@/shared/lib/utils'

import SettingsGroupList from './SettingsGroupList.vue'
import SettingsSections from './SettingsSections.vue'
import { SETTINGS_GROUPS } from './settings-shell.constants'
import { useSettingsNav } from './use-settings-nav'

const { t } = useI18n()

const isDev = import.meta.env.DEV
const { isDesktop } = usePlatform()
/** A finger as the main pointer: no keys to set or list, even in a browser window the size of a phone. */
const coarse = useCoarsePointer()
/** Sections that exist only on a desktop drop out on a phone, and the tabs and groups left empty with them. */
const groups = computed(() =>
  SETTINGS_GROUPS.filter((group) => !group.devOnly || isDev)
    .map((group) => ({
      ...group,
      tabs: group.tabs
        .map((tab) => ({ ...tab, sections: tab.sections.filter((item) => !item.desktopOnly || (isDesktop.value && !coarse.value)) }))
        .filter((tab) => tab.sections.length > 0),
    }))
    .filter((group) => group.tabs.length > 0),
)

const vault = useVaultStore()
// The shell is mounted for the whole unlocked session: it reports the user's activity to the autolock.
useAutolockActivity()
/** Last path segment of the open vault; an Android content:// URI is percent-encoded. */
const vaultFile = computed(() => {
  const last = (vault.info?.path ?? '').split(/[\\/]/).filter(Boolean).pop() ?? ''
  try {
    return decodeURIComponent(last).split(/[\\/:]/).pop() ?? last
  } catch {
    return last
  }
})

const SCROLLER = [
  'min-h-0 flex-1 overflow-y-auto overscroll-contain px-4 pt-4 pb-4 @container',
  'md:scrollbar-stable md:px-6 md:pb-6',
  '[mask-image:linear-gradient(to_bottom,transparent,black_20px)]',
].join(' ')
const scroller = ref<HTMLElement | null>(null)
const { open, isList, activeGroup, activeTab, selectGroup, selectTab, backToList } = useSettingsNav(groups, scroller)

// The window's tour starts from Help only; it closes with the window.
const tour = useTour()
watch(open, (isOpen) => {
  if (!isOpen) tour.stop()
})

const tabDefs = computed(() =>
  activeGroup.value.tabs.map((tab) => ({
    value: tab.id,
    label: t(tab.titleKey),
  })),
)
function onTab(id: string) {
  const tab = activeGroup.value.tabs.find((item) => item.id === id)
  if (tab && tab.id !== activeTab.value.id) selectTab(tab)
}

/** Focus lands on the window itself: Esc and Tab work from inside, and no field opens a keyboard. */
function focusPanel(event: Event) {
  event.preventDefault()
  if (event.currentTarget instanceof HTMLElement) event.currentTarget.focus()
}
</script>

<template>
  <DialogRoot v-model:open="open">
    <DialogPortal>
      <DialogOverlay class="fixed inset-0 z-overlay bg-black/60 data-[state=open]:animate-overlay-in data-[state=closed]:animate-overlay-out" />
      <DialogContent
        :class="cn(
          'fixed inset-0 z-overlay flex overflow-clip bg-card text-card-foreground focus:outline-none',
          'data-[state=open]:animate-overlay-in data-[state=closed]:animate-overlay-out',
          'max-md:flex-col max-md:pt-safe max-md:pb-safe',
          'md:settings-window md:max-h-175 md:max-w-230 md:rounded-xl md:border md:border-border md:shadow-lg',
        )"
        data-tour="settings-shell"
        data-overlay="settings"
        @open-auto-focus="focusPanel"
        @pointer-down-outside="keepOpenOnToast"
        @interact-outside="keepOpenOnToast"
      >
        <!-- Left column of groups; on a phone it is the first screen and hides once a group is open. -->
        <nav
          data-tour="settings-groups"
          :class="cn(
            'flex shrink-0 flex-col gap-2 overflow-y-auto md:scrollbar-stable border-border bg-muted/30 p-3 md:w-70 md:border-r max-md:min-h-0 max-md:flex-1 max-md:bg-card',
            !isList && 'max-md:hidden',
          )"
          :aria-label="t('settingsShell.title')"
        >
          <div class="flex items-center gap-1 md:px-2 md:pb-1">
            <DialogClose as-child>
              <Button variant="ghost" size="icon-sm" class="text-muted-foreground max-md:size-10 md:hidden" :aria-label="t('actions.close')">
                <ChevronLeft class="size-5" />
              </Button>
            </DialogClose>
            <DialogTitle class="max-md:text-xl max-md:font-bold max-md:tracking-tight md:heading-section">{{ t('settingsShell.title') }}</DialogTitle>
          </div>
          <DialogDescription class="sr-only">{{ t('settingsShell.description') }}</DialogDescription>
          <SettingsGroupList :groups="groups" :active-id="activeGroup.id" :list="isList" :file="vaultFile" @select="selectGroup" />
        </nav>

        <div :class="cn('flex min-h-0 min-w-0 flex-1 flex-col', isList && 'max-md:hidden')">
          <header class="flex items-center gap-1 px-4 pt-4 pb-3 md:gap-3 md:px-6 md:pt-5">
            <Button variant="ghost" size="icon-sm" class="text-muted-foreground max-md:size-10 max-md:bg-transparent max-md:hover:bg-transparent md:hidden" :aria-label="t('actions.back')" @click="backToList">
              <ChevronLeft class="size-6" />
            </Button>
            <h2 class="min-w-0 flex-1 truncate max-md:text-xl max-md:font-bold max-md:tracking-tight md:heading-section">{{ t(activeGroup.titleKey) }}</h2>
            <DialogClose as-child>
              <Button variant="ghost" size="icon-sm" class="text-muted-foreground max-md:hidden" :aria-label="t('actions.close')">
                <X class="size-4" />
              </Button>
            </DialogClose>
          </header>

          <!-- The tab strip stays under the header; only the sections below it scroll. -->
          <Tabs
            v-if="tabDefs.length > 1"
            fill
            data-tour="settings-tabs"
            class="shrink-0 px-4 md:px-6"
            :model-value="activeTab.id"
            :tabs="tabDefs"
            @update:model-value="onTab"
          />
          <!-- The first 20px fade out, so text scrolled under the tab strip does not end in a hard cut. -->
          <div
            ref="scroller"
            :class="SCROLLER"
          >
            <SettingsSections :sections="activeTab.sections" />
          </div>
        </div>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>
