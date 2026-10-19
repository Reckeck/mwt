<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { Menu, Plus } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { useSidebar } from '@/shared/ui/sidebar'
import { cn } from '@/shared/lib/utils'

import { NAV_GROUPS, type NavItem } from '../../app-shell.constants'

/**
 * The phone's main sections under the thumb: Home, Wallets, a new
 * transaction, Transactions and «More» (the side menu with the rest).
 */
const emit = defineEmits<{ quickAdd: [] }>()

const { t } = useI18n()
const route = useRoute()
const { setOpenMobile } = useSidebar()

const items = NAV_GROUPS.flatMap((group) => group.items)
const pick = (name: string) => items.find((item) => item.name === name) as NavItem
const left = [pick('dashboard'), pick('stores')]
const right = [pick('transactions')]
const shown = [...left, ...right]

function isActive(item: NavItem): boolean {
  return item.prefix ? route.path.startsWith(item.prefix) : route.name === item.name
}
/** «More» is lit on a section the bar does not show (analytics, plans, workflows). */
const elsewhere = computed(() => !shown.some(isActive))

const ITEM = 'flex min-h-12 flex-1 flex-col items-center justify-center gap-0.5 rounded-md text-[0.6875rem] font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring'
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
    <Button variant="ghost" :class="cn(ITEM, 'h-auto', elsewhere ? 'text-primary' : 'text-muted-foreground')" @click="setOpenMobile(true)">
      <Menu class="size-5" />
      {{ t('nav.more') }}
    </Button>
  </nav>
</template>
