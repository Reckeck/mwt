<script setup lang="ts">
import { computed, watch } from 'vue'
import { RouterLink, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { Lock, Settings } from 'lucide-vue-next'

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
const { setOpenMobile } = useSidebar()
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
        <SidebarMenuItem>
          <SidebarMenuButton as-child size="lg" :tooltip="vaultPath || t('app.title')">
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
