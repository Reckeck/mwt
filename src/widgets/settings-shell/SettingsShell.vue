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
import { EnrichmentSettings } from '@/widgets/enrichment-settings'
import { InflationSettings } from '@/widgets/inflation-settings'
import { NotificationSettings } from '@/widgets/notification-settings'
import { QuickAddSettings } from '@/widgets/quick-add-settings'
import { VaultDevTools } from '@/widgets/vault-dev-tools'
import { VaultJsonEditor } from '@/widgets/vault-json-editor'
import { VaultSettings } from '@/widgets/vault-settings'
import { settingsQuery, withoutSettings } from '@/shared/lib/settings-link'
import { cn } from '@/shared/lib/utils'

import { SETTINGS_TABS } from './settings-shell.constants'
import TourSettings from './TourSettings.vue'

const { t } = useI18n()
const route = useRoute()
const router = useRouter()

const isDev = import.meta.env.DEV
const tabs = SETTINGS_TABS.filter((tab) => !tab.devOnly || isDev)

/** The window lives in the URL: `?settings=<tab>&section=<id>` opens it anywhere, Back closes it. */
const open = computed({
  get: () => typeof route.query.settings === 'string',
  set: (value: boolean) => {
    if (!value) void router.replace({ query: withoutSettings(route.query) })
  },
})
const activeTab = computed(() => tabs.find((tab) => tab.id === route.query.settings) ?? tabs[0])

function selectTab(id: string) {
  void router.replace({ query: settingsQuery(route.query, id) })
}

/* --- sections: clicking or hovering a sub-tab scrolls to it; scrolling lights it up --- */
const scroller = ref<HTMLElement | null>(null)
const visibleSection = ref('')

function scrollTo(id: string, smooth = true) {
  const target = scroller.value?.querySelector<HTMLElement>(`[data-section="${id}"]`)
  target?.scrollIntoView({ behavior: smooth ? 'smooth' : 'auto', block: 'start' })
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
    const wanted = route.query.section
    if (typeof wanted === 'string') scrollTo(wanted, false)
    else scroller.value?.scrollTo({ top: 0 })
  },
  { immediate: true },
)
</script>

<template>
  <DialogRoot v-model:open="open">
    <DialogPortal>
      <DialogOverlay class="fixed inset-0 z-50 bg-black/60 data-[state=open]:animate-overlay-in" />
      <DialogContent
        class="fixed inset-4 z-50 flex overflow-hidden rounded-xl border border-border bg-card text-card-foreground shadow-lg focus:outline-none data-[state=open]:animate-overlay-in lg:inset-[120px]"
        data-tour="settings-shell"
      >
        <!-- Sidebar: tabs; the active one lists its sections. -->
        <nav class="flex w-56 shrink-0 flex-col gap-1 overflow-y-auto border-r border-border bg-muted/30 p-3 max-md:w-14">
          <DialogTitle class="px-2 pb-2 text-sm font-semibold max-md:sr-only">{{ t('settingsShell.title') }}</DialogTitle>
          <DialogDescription class="sr-only">{{ t('settingsShell.description') }}</DialogDescription>
          <template v-for="tab in tabs" :key="tab.id">
            <button
              type="button"
              :class="
                cn(
                  'flex items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm font-medium transition-colors hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
                  activeTab.id === tab.id && 'bg-accent text-accent-foreground',
                )
              "
              :title="t(tab.titleKey)"
              :aria-current="activeTab.id === tab.id ? 'page' : undefined"
              @click="selectTab(tab.id)"
            >
              <component :is="tab.icon" class="size-4 shrink-0 text-muted-foreground" />
              <span class="truncate max-md:sr-only">{{ t(tab.titleKey) }}</span>
            </button>
            <ul v-if="activeTab.id === tab.id && tab.sections.length > 1" class="mb-1 ml-6 grid gap-0.5 max-md:hidden">
              <li v-for="section in tab.sections" :key="section.id">
                <button
                  type="button"
                  :class="
                    cn(
                      'w-full truncate rounded px-2 py-1 text-left text-xs text-muted-foreground transition-colors hover:text-foreground',
                      visibleSection === section.id && 'text-foreground font-medium',
                    )
                  "
                  @click="scrollTo(section.id)"
                  @mouseenter="scrollTo(section.id)"
                >
                  {{ t(section.titleKey) }}
                </button>
              </li>
            </ul>
          </template>
        </nav>

        <div class="flex min-w-0 flex-1 flex-col">
          <header class="flex items-center gap-3 border-b border-border px-5 py-3">
            <h2 class="min-w-0 flex-1 truncate font-semibold">{{ t(activeTab.titleKey) }}</h2>
            <DialogClose
              class="flex size-8 shrink-0 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            >
              <X class="size-4" />
              <span class="sr-only">{{ t('actions.close') }}</span>
            </DialogClose>
          </header>

          <div ref="scroller" class="min-h-0 flex-1 overflow-y-auto p-5 @container">
            <div class="mx-auto grid max-w-3xl gap-6">
              <section
                v-for="section in activeTab.sections"
                :key="section.id"
                :data-section="section.id"
                class="grid scroll-mt-4 gap-3"
              >
                <h3 v-if="activeTab.sections.length > 1" class="text-sm font-semibold text-muted-foreground">
                  {{ t(section.titleKey) }}
                </h3>
                <AppSettingsPanel v-if="section.id === 'app'" />
                <QuickAddSettings v-else-if="section.id === 'quick-add'" />
                <TourSettings v-else-if="section.id === 'tours'" />
                <NotificationSettings v-else-if="section.id === 'notifications'" />
                <VaultSettings v-else-if="section.id === 'vault'" />
                <BudgetSettings v-else-if="section.id === 'budget'" />
                <CategoryManager v-else-if="section.id === 'categories'" />
                <CategorizationRules v-else-if="section.id === 'rules'" />
                <InflationSettings v-else-if="section.id === 'inflation'" />
                <EnrichmentSettings v-else-if="section.id === 'enrichment'" />
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
