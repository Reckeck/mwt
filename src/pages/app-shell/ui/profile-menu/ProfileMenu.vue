<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { CircleUserRound } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { Drawer, DrawerContent, DrawerDescription, DrawerTitle } from '@/shared/ui/drawer'
import { Popover, PopoverContent, PopoverTrigger } from '@/shared/ui/popover'
import { useIsPhone } from '@/shared/lib/phone'

import ProfileMenuList from './ProfileMenuList.vue'
import { useProfileMenu } from './use-profile-menu'

/**
 * The profile menu (Shell · top bar): the vault's actions, Automations, Reminders and Settings.
 * A popover under the icon on a desktop, a bottom sheet on a phone (the sheet replaces «More»).
 */
const { t } = useI18n()
const phone = useIsPhone()
const { open, vaultName, vaultEntries, appEntries } = useProfileMenu()
</script>

<template>
  <Popover :open="!phone && open" @update:open="open = $event">
    <PopoverTrigger as-child>
      <Button
        variant="ghost"
        size="icon"
        class="shrink-0"
        :aria-label="t('profile.open')"
        :title="t('profile.open')"
        data-profile-trigger
      >
        <CircleUserRound class="size-5" />
      </Button>
    </PopoverTrigger>
    <PopoverContent overlay="profile-menu" align="end" class="w-[min(18rem,calc(100vw-2rem))] rounded-sm p-1">
      <ProfileMenuList :vault-name="vaultName" :vault-entries="vaultEntries" :app-entries="appEntries" />
    </PopoverContent>
  </Popover>
  <Drawer :open="phone && open" @update:open="open = $event">
    <DrawerContent layer="profile-menu" class="gap-1 px-3 pt-2 pb-[calc(1rem+var(--safe-bottom))]">
      <DrawerTitle class="sr-only">{{ t('profile.open') }}</DrawerTitle>
      <DrawerDescription class="sr-only">{{ t('profile.hint') }}</DrawerDescription>
      <ProfileMenuList :vault-name="vaultName" :vault-entries="vaultEntries" :app-entries="appEntries" />
    </DrawerContent>
  </Drawer>
</template>
