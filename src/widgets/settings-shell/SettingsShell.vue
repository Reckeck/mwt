<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { X } from 'lucide-vue-next'
import { DialogClose, DialogContent, DialogDescription, DialogOverlay, DialogPortal, DialogRoot, DialogTitle } from 'reka-ui'

import { AppSettingsPanel } from '@/widgets/app-settings'
import { BudgetSettings } from '@/widgets/budget-settings'
import { CategorizationRules } from '@/widgets/categorization-rules'
import { CategoryManager } from '@/widgets/category-manager'
import { CurrencySettings } from '@/widgets/currency-settings'
import { DeviceSettings } from '@/widgets/device-settings'
import { EnrichmentSettings } from '@/widgets/enrichment-settings'
import { FxRates } from '@/widgets/fx-rates'
import { InflationSettings } from '@/widgets/inflation-settings'
import { NotificationSettings } from '@/widgets/notification-settings'
import { QuickAddSettings } from '@/widgets/quick-add-settings'
import { SyncDevices, SyncHistory, SyncSettings, SyncTransfer } from '@/widgets/sync-settings'
import { VaultDevTools } from '@/widgets/vault-dev-tools'
import { VaultDirection } from '@/widgets/vault-direction'
import { VaultJsonEditor } from '@/widgets/vault-json-editor'
import { VaultSettings } from '@/widgets/vault-settings'
import { Button } from '@/shared/ui/button'
import { SettingsSection } from '@/shared/ui/settings-section'
import { settingsQuery, withoutSettings } from '@/shared/lib/settings-link'
import { usePlatform } from '@/shared/lib/platform'
import { cn } from '@/shared/lib/utils'

import { SETTINGS_ALIASES, SETTINGS_TABS } from './settings-shell.constants'
import TourSettings from './TourSettings.vue'
import { keepOpenOnToast } from '@/shared/lib/toast-outside'

const { t } = useI18n()
const route = useRoute()
const router = useRouter()

const isDev = import.meta.env.DEV
const { isDesktop } = usePlatform()
/** Sections that exist only on a desktop drop out on a phone. */
const tabs = computed(() =>
  SETTINGS_TABS.filter((tab) => !tab.devOnly || isDev).map((tab) => ({
    ...tab,
    sections: tab.sections.filter((section) => !section.desktopOnly || isDesktop.value),
  })),
)

/** The window lives in the URL: `?settings=<tab>&section=<id>` opens it anywhere, Back closes it. */
const open = computed({
  get: () => typeof route.query.settings === 'string',
  set: (value: boolean) => {
    if (!value) void router.replace({ query: withoutSettings(route.query) })
  },
})
const alias = computed(() => SETTINGS_ALIASES[String(route.query.settings)])
const activeTab = computed(
  () => tabs.value.find((tab) => tab.id === (alias.value?.tab ?? route.query.settings)) ?? tabs.value[0],
)

function selectTab(id: string) {
  void router.replace({ query: settingsQuery(route.query, id) })
}

/* --- sections: clicking a sub-tab scrolls to it; scrolling lights it up --- */
const scroller = ref<HTMLElement | null>(null)
const visibleSection = ref('')

function scrollTo(id: string, smooth = true) {
  const target = scroller.value?.querySelector<HTMLElement>(`[data-section="${id}"]`)
  const reduce = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches
  target?.scrollIntoView({ behavior: smooth && !reduce ? 'smooth' : 'auto', block: 'start' })
}

let observer: IntersectionObserver | null = null
function observeSections() {
  observer?.disconnect()
  const root = scroller.value
  if (!root) return
  observer = new IntersectionObserver(
    (entries) => {
      const shown = entries.filter((entry) => entry.isIntersecting)
      if (shown.length) visibleSection.value = (shown[0].target as HTMLElement).dataset.section ?? ''
    },
    { root, rootMargin: '0px 0px -70% 0px' },
  )
  for (const section of root.querySelectorAll('[data-section]')) observer.observe(section)
}

watch(
  [open, () => activeTab.value.id],
  async ([isOpen]) => {
    if (!isOpen) {
      observer?.disconnect()
      return
    }
    await nextTick()
    visibleSection.value = activeTab.value.sections[0]?.id ?? ''
    observeSections()
    const wanted = alias.value?.section ?? route.query.section
    if (typeof wanted === 'string') scrollTo(wanted, false)
    else scroller.value?.scrollTo({ top: 0 })
  },
  { immediate: true },
)

/** Focus lands on the open tab, not the first one: two lit tabs would say two things. */
const nav = ref<HTMLElement | null>(null)
function focusActiveTab(event: Event) {
  event.preventDefault()
  void nextTick(() => nav.value?.querySelector<HTMLElement>('[aria-current="page"]')?.focus())
}
</script>

<template>
  <DialogRoot v-model:open="open">
    <DialogPortal>
      <DialogOverlay class="fixed inset-0 z-overlay bg-black/60 data-[state=open]:animate-overlay-in data-[state=closed]:animate-overlay-out" />
      <DialogContent
        class="fixed inset-0 z-overlay flex overflow-hidden bg-card text-card-foreground focus:outline-none data-[state=open]:animate-overlay-in data-[state=closed]:animate-overlay-out max-md:flex-col max-md:pt-safe max-md:pb-safe md:settings-window md:rounded-xl md:border md:border-border md:shadow-lg"
        data-tour="settings-shell"
        @open-auto-focus="focusActiveTab"
        @pointer-down-outside="keepOpenOnToast"
        @interact-outside="keepOpenOnToast"
      >
        <!-- Sidebar: tabs; the active one lists its sections. A phone shows a strip of tabs under the title. -->
        <nav
          ref="nav"
          class="flex w-full shrink-0 flex-col gap-1 overflow-y-auto scrollbar-stable border-r border-border bg-muted/30 p-3 max-md:order-2 md:w-56 max-md:flex-row max-md:overflow-x-auto max-md:border-r-0 max-md:border-b max-md:p-2 max-md:scrollbar-none"
        >
          <DialogTitle class="px-2 pb-2 text-sm font-semibold max-md:sr-only">{{ t('settingsShell.title') }}</DialogTitle>
          <DialogDescription class="sr-only">{{ t('settingsShell.description') }}</DialogDescription>
          <template v-for="tab in tabs" :key="tab.id">
            <Button
              variant="ghost"
              size="block"
              :class="cn('gap-2 font-medium max-md:w-auto max-md:shrink-0', activeTab.id === tab.id && 'bg-accent text-accent-foreground')"
              :title="t(tab.titleKey)"
              :aria-current="activeTab.id === tab.id ? 'page' : undefined"
              @click="selectTab(tab.id)"
            >
              <component :is="tab.icon" class="size-4 shrink-0 text-muted-foreground" />
              <span class="truncate">{{ t(tab.titleKey) }}</span>
            </Button>
          </template>
        </nav>

        <div class="flex min-w-0 flex-1 flex-col max-md:contents">
          <header class="grid gap-2 border-b border-border px-4 py-3 max-md:order-1">
            <div class="flex items-center gap-3">
              <h2 class="heading-section min-w-0 flex-1 truncate">{{ t(activeTab.titleKey) }}</h2>
              <DialogClose as-child>
                <Button variant="ghost" size="icon-sm" class="max-md:size-10" :aria-label="t('actions.close')">
                  <X class="size-4" />
                </Button>
              </DialogClose>
            </div>
            <!-- The tab's sections live here, not under the tab in the sidebar: a list that
                 opened there pushed the tabs below it away from the pointer (rule 17). -->
            <nav
              v-if="activeTab.sections.length > 1"
              class="-mx-1 flex gap-1 overflow-x-auto scrollbar-none max-md:hidden"
              :aria-label="t(activeTab.titleKey)"
            >
              <Button
                v-for="section in activeTab.sections"
                :key="section.id"
                variant="ghost"
                size="sm"
                :class="cn('shrink-0 font-normal text-muted-foreground', visibleSection === section.id && 'bg-accent text-foreground')"
                :aria-current="visibleSection === section.id ? 'location' : undefined"
                @click="scrollTo(section.id)"
              >
                {{ t(section.titleKey) }}
              </Button>
            </nav>
          </header>

          <div ref="scroller" class="min-h-0 flex-1 overflow-y-auto overscroll-contain scrollbar-stable p-4 @container max-md:order-3">
            <div class="mx-auto grid max-w-3xl grid-cols-1 gap-4">
              <section
                v-for="section in activeTab.sections"
                :key="section.id"
                :data-section="section.id"
                class="grid min-w-0 scroll-mt-4 grid-cols-1"
              >
                <SettingsSection v-if="section.id === 'app'" :title="t('settingsShell.section.app')" :description="t('settings.appScopeHint')"><AppSettingsPanel /></SettingsSection>
                <DeviceSettings v-else-if="section.id === 'device-app'" />
                <QuickAddSettings v-else-if="section.id === 'quick-add'" />
                <TourSettings v-else-if="section.id === 'tours'" />
                <SettingsSection v-else-if="section.id === 'notifications'" :title="t('settingsShell.section.notifications')"><NotificationSettings /></SettingsSection>
                <VaultSettings v-else-if="section.id === 'vault'" />
                <VaultDirection v-else-if="section.id === 'direction'" />
                <CurrencySettings v-else-if="section.id === 'currencies'" />
                <FxRates v-else-if="section.id === 'rates'" />
                <BudgetSettings v-else-if="section.id === 'budget'" />
                <CategoryManager v-else-if="section.id === 'categories'" />
                <CategorizationRules v-else-if="section.id === 'rules'" />
                <InflationSettings v-else-if="section.id === 'inflation'" />
                <EnrichmentSettings v-else-if="section.id === 'enrichment'" />
                <SyncSettings v-else-if="section.id === 'sync'" />
                <SyncDevices v-else-if="section.id === 'sync-devices'" />
                <SyncHistory v-else-if="section.id === 'sync-history'" />
                <SyncTransfer v-else-if="section.id === 'sync-transfer'" />
                <VaultDevTools v-else-if="section.id === 'devtools'">
                  <template #viewer>
                    <VaultJsonEditor />
                  </template>
                </VaultDevTools>
              </section>
            </div>
          </div>
        </div>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>
