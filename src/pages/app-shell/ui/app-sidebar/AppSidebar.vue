<script setup lang="ts">
import { computed, watch } from 'vue'
import { RouterLink, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { Lock, PanelLeftClose, PanelLeftOpen, Settings } from 'lucide-vue-next'

import { SidebarEvents } from '@/widgets/sidebar-events'
import { SidebarPinned } from '@/widgets/sidebar-pinned'
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
  SidebarSeparator,
  useSidebar,
} from '@/shared/ui/sidebar'
import { hotkeyLabel } from '@/shared/lib/hotkeys'
import { BrandMark } from '@/shared/ui/brand-mark'
import { useVaultStore } from '@/shared/store/vault'

import { NAV_ITEMS, type NavItem } from '../../app-shell.constants'

/**
 * Shell · 7: the five sections, the favourites, then settings, lock and
 * «Свернуть · Ctrl B». Folded to icons, every row keeps a tooltip.
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

const isActive = (item: NavItem) => (item.prefix ? route.path.startsWith(item.prefix) : route.name === item.name)
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
            <SidebarMenuItem v-for="item in NAV_ITEMS" :key="item.name">
              <SidebarMenuButton as-child :is-active="isActive(item)" :tooltip="t(item.key)">
                <RouterLink :to="{ name: item.name }">
                  <component :is="item.icon" />
                  <span>{{ t(item.key) }}</span>
                </RouterLink>
              </SidebarMenuButton>
            </SidebarMenuItem>
          </SidebarMenu>
        </SidebarGroupContent>
      </SidebarGroup>

      <SidebarSeparator v-if="collapsed" class="my-1" />
      <SidebarPinned />
      <SidebarEvents />
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
