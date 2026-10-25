<script setup lang="ts">
import { computed, ref } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ChevronRight, Lock, Menu, Plus, Settings } from 'lucide-vue-next'

import { useStoreTree } from '@/shared/api/queries'
import { Button } from '@/shared/ui/button'
import { Drawer, DrawerContent, DrawerDescription, DrawerHeader, DrawerTitle } from '@/shared/ui/drawer'
import { cn } from '@/shared/lib/utils'
import { useVaultStore } from '@/shared/store/vault'

import { type NavItem, navItem } from '../../app-shell.constants'

/**
 * The phone's bar (Shell · 6): Home, Transactions, a new transaction, Money and
 * «More» — a sheet with wallets, automations, settings and the lock.
 */
const emit = defineEmits<{ quickAdd: []; lock: []; openSettings: [] }>()
const vault = useVaultStore()

const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const tree = useStoreTree()

const left = [navItem('dashboard'), navItem('transactions')]
const right = [navItem('money')]
const rest = [navItem('stores'), navItem('automations')]

function isActive(item: NavItem): boolean {
  return item.prefix ? route.path.startsWith(item.prefix) : route.name === item.name
}
/** «More» is lit on a section the bar does not show. */
const elsewhere = computed(() => rest.some(isActive))
const walletCount = computed(() => tree.data.value?.length ?? 0)

const moreOpen = ref(false)
function pick(run: () => void) {
  moreOpen.value = false
  run()
}

const ITEM = 'flex min-h-12 flex-1 flex-col items-center justify-center gap-0.5 rounded-md text-[0.6875rem] font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring'
const ROW = 'h-auto min-h-12 w-full justify-start gap-3 rounded-none border-b px-1 text-left text-sm font-normal last:border-0'
</script>

<template>
  <nav
    class="flex shrink-0 items-stretch gap-1 border-t border-border bg-card px-2 pt-1 pb-[calc(0.25rem+var(--safe-bottom))]"
    :aria-label="t('nav.bottom')"
    data-tour="bottom-nav"
  >
    <RouterLink
      v-for="item in left"
      :key="item.name"
      :to="{ name: item.name }"
      :class="cn(ITEM, isActive(item) ? 'text-primary' : 'text-muted-foreground')"
      :aria-current="isActive(item) ? 'page' : undefined"
    >
      <component :is="item.icon" class="size-5" />
      {{ t(item.key) }}
    </RouterLink>
    <Button variant="ghost" :class="cn(ITEM, 'h-auto text-muted-foreground')" :aria-label="t('quickAdd.title')" @click="emit('quickAdd')">
      <span class="flex size-9 items-center justify-center rounded-full bg-primary text-primary-foreground shadow">
        <Plus class="size-5" />
      </span>
    </Button>
    <RouterLink
      v-for="item in right"
      :key="item.name"
      :to="{ name: item.name }"
      :class="cn(ITEM, isActive(item) ? 'text-primary' : 'text-muted-foreground')"
      :aria-current="isActive(item) ? 'page' : undefined"
    >
      <component :is="item.icon" class="size-5" />
      {{ t(item.key) }}
    </RouterLink>
    <Button
      variant="ghost"
      :class="cn(ITEM, 'h-auto', elsewhere || moreOpen ? 'text-primary' : 'text-muted-foreground')"
      data-bottom-more
      @click="moreOpen = true"
    >
      <Menu class="size-5" />
      {{ t('nav.more') }}
    </Button>
  </nav>

  <Drawer v-model:open="moreOpen">
    <DrawerContent layer="bottom-more" data-bottom-more-sheet>
      <DrawerHeader class="px-4 pb-0 text-left">
        <DrawerTitle>{{ t('nav.more') }}</DrawerTitle>
        <DrawerDescription class="sr-only">{{ t('nav.moreHint') }}</DrawerDescription>
      </DrawerHeader>
      <div class="grid px-4 pb-[calc(1.5rem+var(--safe-bottom))]">
        <Button v-for="item in rest" :key="item.name" variant="ghost" :class="ROW" @click="pick(() => router.push({ name: item.name }))">
          <component :is="item.icon" class="size-5 text-muted-foreground" />
          <span class="flex-1">{{ t(item.key) }}</span>
          <span v-if="item.name === 'stores' && walletCount" class="text-muted-foreground">
            {{ t('nav.walletCount', { count: walletCount }) }}
          </span>
          <ChevronRight class="size-4 text-muted-foreground" />
        </Button>
        <Button variant="ghost" :class="ROW" @click="pick(() => emit('openSettings'))">
          <Settings class="size-5 text-muted-foreground" />
          <span class="flex-1">{{ t('settingsShell.title') }}</span>
          <ChevronRight class="size-4 text-muted-foreground" />
        </Button>
        <Button v-if="vault.encrypted" variant="ghost" :class="ROW" @click="pick(() => emit('lock'))">
          <Lock class="size-5 text-muted-foreground" />
          <span class="flex-1">{{ t('auth.lock') }}</span>
        </Button>
      </div>
    </DrawerContent>
  </Drawer>
</template>
