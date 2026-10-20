<script setup lang="ts">
import { computed, watch } from 'vue'
import { RouterLink, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { Lock, PanelLeft, Settings } from 'lucide-vue-next'

import { SidebarWallets } from '@/widgets/sidebar-wallets'
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarRail,
  SidebarSeparator,
  useSidebar,
} from '@/shared/ui/sidebar'
import { Button } from '@/shared/ui/button'
import { useVaultStore } from '@/shared/store/vault'

import { NAV_GROUPS, type NavItem } from '../../app-shell.constants'

/** The app's sections, then the wallets; settings and lock at the bottom. */
const emit = defineEmits<{ lock: []; openSettings: [] }>()

const { t } = useI18n()
const route = useRoute()
const vault = useVaultStore()

/** File name of the open vault; the full path is in the tooltip. */
const vaultPath = computed(() => vault.status?.path ?? '')
const vaultFile = computed(() => vaultPath.value.split(/[\\/]/).pop() ?? '')

/** On a phone the menu is a sheet over the page: a picked section closes it. */
const { setOpenMobile, state, isMobile, toggleSidebar } = useSidebar()
/** The desktop rail of icons: the logo is the «expand» button there. */
const collapsed = computed(() => state.value === 'collapsed' && !isMobile.value)
watch(
  () => route.fullPath,
  () => setOpenMobile(false),
)

const isActive = (item: NavItem) => (item.prefix ? route.path.startsWith(item.prefix) : route.name === item.name)
</script>

<template>
  <Sidebar collapsible="icon">
    <SidebarHeader>
      <SidebarMenu>
        <!-- Collapsed: the logo expands the sidebar; with a mouse it turns into the panel icon on hover. -->
        <SidebarMenuItem v-if="collapsed">
          <SidebarMenuButton
            size="lg"
            class="group/logo"
            :tooltip="t('sidebar.expand')"
            :aria-label="t('sidebar.expand')"
            data-tour="sidebar-toggle"
            @click="toggleSidebar()"
          >
            <div class="relative flex aspect-square size-8 items-center justify-center rounded-lg bg-sidebar-primary text-sm font-bold text-sidebar-primary-foreground">
              <span
                class="transition-opacity pointer-coarse:opacity-0 pointer-fine:group-hover/logo:opacity-0 pointer-fine:group-focus-visible/logo:opacity-0"
                aria-hidden="true"
              >
                M
              </span>
              <PanelLeft
                class="absolute size-4 transition-opacity pointer-fine:opacity-0 pointer-fine:group-hover/logo:opacity-100 pointer-fine:group-focus-visible/logo:opacity-100"
                aria-hidden="true"
              />
            </div>
          </SidebarMenuButton>
        </SidebarMenuItem>
        <!-- Expanded: the logo opens the home page, the button beside it folds the sidebar. -->
        <SidebarMenuItem v-else class="flex items-center gap-1">
          <SidebarMenuButton as-child size="lg" class="min-w-0 flex-1" :tooltip="vaultPath || t('app.title')">
            <RouterLink :to="{ name: 'dashboard' }">
              <div
                class="flex aspect-square size-8 items-center justify-center rounded-lg bg-sidebar-primary text-sm font-bold text-sidebar-primary-foreground"
                aria-hidden="true"
              >
                M
              </div>
              <div class="grid flex-1 text-left leading-tight">
                <span class="truncate font-semibold">{{ t('app.title') }}</span>
                <span class="truncate text-xs text-muted-foreground" :title="vaultPath">{{ vaultFile }}</span>
              </div>
            </RouterLink>
          </SidebarMenuButton>
          <Button
            v-if="!isMobile"
            variant="ghost"
            size="icon-sm"
            class="shrink-0 text-muted-foreground"
            :title="t('sidebar.collapse')"
            :aria-label="t('sidebar.collapse')"
            data-tour="sidebar-toggle"
            @click="toggleSidebar()"
          >
            <PanelLeft />
          </Button>
        </SidebarMenuItem>
      </SidebarMenu>
    </SidebarHeader>

    <SidebarContent class="gap-0">
      <SidebarGroup v-for="group in NAV_GROUPS" :key="group.key ?? 'main'" class="py-1">
        <SidebarGroupLabel v-if="group.key" class="h-6 group-data-[collapsible=icon]:-mt-6">{{ t(group.key) }}</SidebarGroupLabel>
        <SidebarGroupContent>
          <SidebarMenu>
            <SidebarMenuItem v-for="item in group.items" :key="item.name">
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

      <SidebarSeparator class="my-1" />

      <SidebarWallets />
    </SidebarContent>

    <SidebarFooter>
      <SidebarMenu>
        <SidebarMenuItem>
          <SidebarMenuButton :tooltip="t('settingsShell.title')" data-tour="open-settings" @click="emit('openSettings')">
            <Settings />
            <span>{{ t('settingsShell.title') }}</span>
          </SidebarMenuButton>
        </SidebarMenuItem>
        <SidebarMenuItem>
          <SidebarMenuButton :tooltip="t('auth.lock')" @click="emit('lock')">
            <Lock />
            <span>{{ t('auth.lock') }}</span>
          </SidebarMenuButton>
        </SidebarMenuItem>
      </SidebarMenu>
    </SidebarFooter>

    <SidebarRail />
  </Sidebar>
</template>
