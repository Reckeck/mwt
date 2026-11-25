<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { CircleUserRound } from 'lucide-vue-next'

import { NotificationBell } from '@/widgets/notification-bell'
import { Button } from '@/shared/ui/button'
import { Drawer, DrawerContent, DrawerDescription, DrawerTitle } from '@/shared/ui/drawer'
import { Popover, PopoverContent, PopoverTrigger } from '@/shared/ui/popover'
import { useIsPhone } from '@/shared/lib/phone'

import ProfileMenuList from './ProfileMenuList.vue'
import { useProfileMenu } from './use-profile-menu'

const props = defineProps<{
  /** This screen has a tour, and its step count. */
  hasTour: boolean
  tourSteps?: number
}>()
const emit = defineEmits<{ startTour: []; allTours: [] }>()

/**
 * The profile menu (Shell · top bar): the vault's actions, sync, notifications and tours (moved
 * out of the top bar), Automations, Reminders and Settings. A dot on the button says something
 * needs a look: unread notifications or a sync error.
 * A popover under the icon on a desktop, a bottom sheet on a phone (the sheet replaces «More»).
 */
const { t } = useI18n()
const phone = useIsPhone()
const { open, noticesOpen, attention, vaultName, vaultEntries, statusEntries, appEntries } = useProfileMenu({
  tour: () => ({ available: props.hasTour, steps: props.tourSteps ?? 0 }),
  startTour: () => emit('startTour'),
  allTours: () => emit('allTours'),
})
</script>

<template>
  <Popover :open="!phone && open" @update:open="open = $event">
    <PopoverTrigger as-child>
      <Button
        variant="ghost"
        size="icon"
        :aria-label="t('profile.open')"
        :title="t('profile.open')"
        class="relative shrink-0"
        data-profile-trigger
        data-tour="profile-trigger"
      >
        <CircleUserRound class="size-5" />
        <span
          v-if="attention"
          class="absolute right-2 top-2 size-2 rounded-full bg-destructive ring-2 ring-background"
          data-profile-dot
        />
      </Button>
    </PopoverTrigger>
    <PopoverContent overlay="profile-menu" align="end" class="w-[min(18rem,calc(100vw-2rem))] rounded-sm p-1">
      <ProfileMenuList :vault-name="vaultName" :vault-entries="vaultEntries" :status-entries="statusEntries" :app-entries="appEntries" />
    </PopoverContent>
  </Popover>
  <Drawer :open="phone && open" @update:open="open = $event">
    <DrawerContent layer="profile-menu" class="gap-1 px-3 pt-2 pb-[calc(1rem+var(--safe-bottom))]">
      <DrawerTitle class="sr-only">{{ t('profile.open') }}</DrawerTitle>
      <DrawerDescription class="sr-only">{{ t('profile.hint') }}</DrawerDescription>
      <ProfileMenuList :vault-name="vaultName" :vault-entries="vaultEntries" :status-entries="statusEntries" :app-entries="appEntries" />
    </DrawerContent>
  </Drawer>
  <NotificationBell v-model:open="noticesOpen" />
</template>
