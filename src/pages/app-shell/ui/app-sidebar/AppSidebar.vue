<script setup lang="ts">
import { computed, watch } from 'vue'
import { RouterLink, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { Lock, PanelLeftClose, PanelLeftOpen, Plus, Settings } from 'lucide-vue-next'

import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarRail,
  useSidebar,
} from '@/shared/ui/sidebar'
import { hotkeyLabel } from '@/shared/lib/hotkeys'
import { openQuickAdd } from '@/shared/lib/quick-add'
import { BrandMark } from '@/shared/ui/brand-mark'
import { useVaultStore } from '@/shared/store/vault'

import { NAV_SECTIONS, sectionOfRoute } from '../../lib/nav-sections'

/**
 * Shell · 7: the brand block, «New operation», the five sections, then settings, lock and
 * `Collapse · Ctrl B`. Folded to icons, every row keeps a tooltip. The wallet, pinned and events
 * groups are gone (spec D9): wallets live in Капитал, events in Люди, pinned items in the palette.
 */
const emit = defineEmits<{ lock: []; openSettings: [] }>()

const { t } = useI18n()
const route = useRoute()
const vault = useVaultStore()

/** File name of the open vault; the full path is in the tooltip. */
const vaultPath = computed(() => vault.status?.path ?? '')
const vaultFile = computed(() =>
  vault.sandbox ? t('sandbox.sidebar') : (vaultPath.value.split(/[\\/]/).pop() ?? ''),
)

const { setOpenMobile, setOpen, state, isMobile, narrow, toggleSidebar } = useSidebar()
const collapsed = computed(() => state.value === 'collapsed' && !isMobile.value)
const foldLabel = computed(() => `${collapsed.value ? t('sidebar.expand') : t('sidebar.collapse')} · ${hotkeyLabel('sidebar')}`)
/** A picked section closes the phone's sheet, and the sidebar lying over a narrow window. */
watch(
  () => route.fullPath,
  () => {
    setOpenMobile(false)
    if (narrow.value) setOpen(false)
  },
)

const activeSection = computed(() => sectionOfRoute(String(route.name ?? ''))?.id)

/** Section ids whose tour anchor stays on the item. */
const TOUR_TARGETS: Partial<Record<string, string>> = { people: 'open-contacts' }
</script>

<template>
  <Sidebar collapsible="icon">
    <SidebarHeader>
      <SidebarMenu>
        <SidebarMenuItem>
          <SidebarMenuButton as-child size="lg" :tooltip="vaultPath || t('app.title')">
            <RouterLink :to="{ name: 'dashboard' }">
              <BrandMark class="size-8" />
              <div class="grid flex-1 text-left leading-tight">
                <span class="truncate font-semibold">{{ t('app.title') }}</span>
                <span class="truncate text-xs text-muted-foreground" :title="vaultPath">{{ vaultFile }}</span>
              </div>
            </RouterLink>
          </SidebarMenuButton>
        </SidebarMenuItem>
      </SidebarMenu>
    </SidebarHeader>

    <SidebarContent class="gap-0">
      <SidebarGroup class="py-1">
        <SidebarGroupContent>
          <SidebarMenu>
            <SidebarMenuItem>
              <SidebarMenuButton
                class="mb-1 bg-primary text-primary-foreground hover:bg-primary/90 hover:text-primary-foreground"
                :tooltip="`${t('quickAdd.title')} · ${hotkeyLabel('quickAdd')}`"
                @click="openQuickAdd()"
              >
                <Plus />
                <span class="flex-1">{{ t('quickAdd.title') }}</span>
                <kbd class="text-xs opacity-80">{{ hotkeyLabel('quickAdd') }}</kbd>
              </SidebarMenuButton>
            </SidebarMenuItem>
            <SidebarMenuItem v-for="section in NAV_SECTIONS" :key="section.id">
              <SidebarMenuButton
                as-child
                :is-active="activeSection === section.id"
                :tooltip="t(section.key)"
                :data-tour="TOUR_TARGETS[section.id]"
              >
                <RouterLink :to="{ name: section.root }" :aria-current="activeSection === section.id ? 'page' : undefined">
                  <component :is="section.icon" />
                  <span>{{ t(section.key) }}</span>
                </RouterLink>
              </SidebarMenuButton>
            </SidebarMenuItem>
          </SidebarMenu>
        </SidebarGroupContent>
      </SidebarGroup>
    </SidebarContent>

    <SidebarFooter>
      <SidebarMenu>
        <SidebarMenuItem>
          <SidebarMenuButton :tooltip="t('settingsShell.title')" data-tour="open-settings" @click="emit('openSettings')">
            <Settings />
            <span>{{ t('settingsShell.title') }}</span>
          </SidebarMenuButton>
        </SidebarMenuItem>
        <!-- A vault without a password has nothing to lock with (Full-New-Encrypt 3). -->
        <SidebarMenuItem v-if="vault.encrypted">
          <SidebarMenuButton :tooltip="t('auth.lock')" @click="emit('lock')">
            <Lock />
            <span>{{ t('auth.lock') }}</span>
          </SidebarMenuButton>
        </SidebarMenuItem>
        <SidebarMenuItem v-if="!isMobile">
          <SidebarMenuButton
            class="text-muted-foreground"
            :tooltip="foldLabel"
            :aria-label="foldLabel"
            data-tour="sidebar-toggle"
            @click="toggleSidebar()"
          >
            <PanelLeftOpen v-if="collapsed" />
            <PanelLeftClose v-else />
            <span class="flex-1">{{ collapsed ? t('sidebar.expand') : t('sidebar.collapse') }}</span>
            <kbd class="text-xs">{{ hotkeyLabel('sidebar') }}</kbd>
          </SidebarMenuButton>
        </SidebarMenuItem>
      </SidebarMenu>
    </SidebarFooter>

    <SidebarRail />
  </Sidebar>
</template>
