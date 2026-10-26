<script setup lang="ts">
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { CircleHelp, Plus, WifiOff } from 'lucide-vue-next'

import { SyncIndicator } from '@/features/vault-sync'
import { Money } from '@/entities/currency'
import { NotificationBell } from '@/widgets/notification-bell'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { DropdownMenu } from '@/shared/ui/dropdown-menu'
import { DropdownMenuItem } from '@/shared/ui/dropdown-menu-item'
import { Kbd } from '@/shared/ui/kbd'
import { hotkeyLabel } from '@/shared/lib/hotkeys'
import { pluralForm } from '@/shared/lib/format'

/** The top bar: where you are, whether the app is online, the net worth, and the global actions. */
const props = defineProps<{
  title: string
  summary: PortfolioSummary | undefined
  online: boolean
  /** The screen has a tour to start with «?». */
  hasTour: boolean
  /** Steps of that tour, said in the «?» menu. */
  tourSteps?: number
  /** Something above (the sandbox bar) already clears the status bar. */
  covered?: boolean
}>()
const emit = defineEmits<{ quickAdd: []; startTour: []; allTours: [] }>()

const { t, locale } = useI18n()
</script>

<template>
  <header
    class="relative box-content flex h-14 shrink-0 items-center gap-2 border-b border-border px-3 max-md:gap-1 max-md:pr-2 md:px-4"
    :class="!props.covered && 'pt-safe'"
  >
    <!-- The sidebar folds from its own logo (AppSidebar); a phone reaches it from «More». -->
    <h1 class="heading-page min-w-0 flex-1 truncate">{{ props.title }}</h1>

    <Badge v-if="!props.online" variant="destructive" class="gap-1">
      <WifiOff class="size-3" />
      {{ t('app.offline') }}
    </Badge>

    <!-- Net worth, the same figure as the dashboard and the «What I have» report it opens. -->
    <RouterLink
      v-if="props.summary"
      :to="{ name: 'money-wealth' }"
      class="hidden rounded-md px-1 text-sm font-medium hover:bg-accent sm:inline"
      :aria-label="t('header.netWorthHint')"
    >
      <Money :amount="props.summary.net_worth_base" :currency="props.summary.base_currency" :decimals="2" />
    </RouterLink>

    <SyncIndicator />
    <NotificationBell />
    <slot name="search" />
    <Button size="sm" class="shrink-0 max-md:hidden" :title="t('quickAdd.title')" data-tour="quick-add" @click="emit('quickAdd')">
      <Plus class="size-4" />
      <span class="hidden sm:inline">{{ t('quickAdd.button') }}</span>
      <Kbd class="hidden border-primary-foreground/30 bg-primary-foreground/15 text-primary-foreground xl:inline-flex">{{ hotkeyLabel('quickAdd') }}</Kbd>
    </Button>

    <!-- «?»: this screen's tour, or all of them in Settings › Help. -->
    <DropdownMenu v-if="props.hasTour" align="end">
      <template #trigger>
        <Button
          variant="ghost"
          size="icon"
          class="shrink-0"
          :aria-label="t('tour.start')"
          :title="t('tour.start')"
          data-tour="tour-help"
        >
          <CircleHelp class="size-4" />
        </Button>
      </template>
      <DropdownMenuItem value="tour" class="min-h-10" @select="emit('startTour')">
        <span class="grid">
          <b class="font-medium">{{ t('tour.menuShow') }}</b>
          <span class="text-xs text-muted-foreground">{{ t(`tour.menuShowHint.${pluralForm(props.tourSteps ?? 0, locale)}`, { count: props.tourSteps ?? 0 }) }}</span>
        </span>
      </DropdownMenuItem>
      <DropdownMenuItem value="all" class="min-h-10" @select="emit('allTours')">
        <span class="grid">
          <b class="font-medium">{{ t('tour.menuAll') }}</b>
          <span class="text-xs text-muted-foreground">{{ t('tour.menuAllHint') }}</span>
        </span>
      </DropdownMenuItem>
    </DropdownMenu>
  </header>
</template>
