<script setup lang="ts">
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { CircleHelp, Plus, WifiOff } from 'lucide-vue-next'

import { SyncIndicator } from '@/features/vault-sync'
import { Money } from '@/entities/currency'
import { NotificationBell } from '@/widgets/notification-bell'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { Separator } from '@/shared/ui/separator'
import { SidebarTrigger } from '@/shared/ui/sidebar'

/** The top bar: where you are, whether the app is online, the net worth, and the global actions. */
const props = defineProps<{
  title: string
  summary: PortfolioSummary | undefined
  online: boolean
  /** The screen has a tour to start with «?». */
  hasTour: boolean
}>()
const emit = defineEmits<{ quickAdd: []; startTour: [] }>()

const { t } = useI18n()
</script>

<template>
  <header class="relative box-content flex h-14 shrink-0 items-center gap-2 border-b border-border px-4 pt-safe max-md:gap-1 max-md:pr-2 md:px-6">
    <!-- A phone reaches the menu from «More» in the bottom bar. -->
    <SidebarTrigger class="-ml-1 max-md:hidden" />
    <Separator orientation="vertical" class="mr-2 data-[orientation=vertical]:h-4 max-md:hidden" />
    <h1 class="heading-page min-w-0 flex-1 truncate">{{ props.title }}</h1>

    <Badge v-if="!props.online" variant="destructive" class="gap-1">
      <WifiOff class="size-3" />
      {{ t('app.offline') }}
    </Badge>

    <!-- Net worth, the same figure as the dashboard and the «What I have» report it opens. -->
    <RouterLink
      v-if="props.summary"
      :to="{ name: 'analytics-wealth' }"
      class="hidden rounded-md px-1 text-sm font-medium hover:bg-accent sm:inline"
      :aria-label="t('header.netWorthHint')"
    >
      <Money :amount="props.summary.net_worth_base" :currency="props.summary.base_currency" :decimals="2" />
    </RouterLink>

    <slot name="search" />
    <Button size="sm" class="shrink-0 max-md:hidden" :title="t('quickAdd.title')" data-tour="quick-add" @click="emit('quickAdd')">
      <Plus class="size-4" />
      <span class="hidden sm:inline">{{ t('quickAdd.button') }}</span>
    </Button>

    <SyncIndicator />
    <NotificationBell />
    <Button
      v-if="props.hasTour"
      variant="ghost"
      size="icon"
      class="shrink-0"
      :aria-label="t('tour.start')"
      :title="t('tour.start')"
      data-tour="tour-help"
      @click="emit('startTour')"
    >
      <CircleHelp class="size-4" />
    </Button>
  </header>
</template>
