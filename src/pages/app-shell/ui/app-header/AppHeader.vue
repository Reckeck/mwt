<script setup lang="ts">
import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ArrowLeft, WifiOff } from 'lucide-vue-next'

import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { useIsPhone } from '@/shared/lib/phone'

import { showBack as showsBack } from '../../lib/show-back'
import { ProfileMenu } from '../profile-menu'

/**
 * The top bar: on a phone Back (off a section root), the title, search and the profile icon; on a
 * desktop the title, the search field with the `Ctrl K` hint and the profile button. Sync,
 * notifications, tours and the net worth live in the profile menu; only the offline state stays
 * here, as a small badge.
 */
const props = defineProps<{
  title: string
  online: boolean
  /** The screen has a tour, started from the profile menu. */
  hasTour: boolean
  /** Steps of that tour, said in the profile menu. */
  tourSteps?: number
  /** Something above (the sandbox bar) already clears the status bar. */
  covered?: boolean
}>()
const emit = defineEmits<{ startTour: []; allTours: [] }>()

const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const phone = useIsPhone()

const showBack = computed(() => showsBack(phone.value, route.name))

function goBack() {
  if (window.history.length > 1) router.back()
  else void router.push({ name: 'dashboard' })
}
</script>

<template>
  <header
    class="relative box-content flex h-14 shrink-0 items-center gap-2 border-b border-border px-3 max-md:gap-1 max-md:pr-2 md:px-4"
    :class="!props.covered && 'pt-safe'"
  >
    <!-- The sidebar folds from its own logo (AppSidebar); a phone has no sidebar. -->
    <Button v-if="showBack" variant="ghost" size="icon" class="shrink-0" :aria-label="t('actions.back')" data-header-back @click="goBack">
      <ArrowLeft class="size-5" />
    </Button>
    <h1 class="heading-page min-w-0 flex-1 truncate">{{ props.title }}</h1>

    <Badge v-if="!props.online" variant="destructive" class="gap-1">
      <WifiOff class="size-3" />
      {{ t('app.offline') }}
    </Badge>

    <slot name="search" />
    <ProfileMenu
      :has-tour="props.hasTour"
      :tour-steps="props.tourSteps"
      @start-tour="emit('startTour')"
      @all-tours="emit('allTours')"
    />
  </header>
</template>
