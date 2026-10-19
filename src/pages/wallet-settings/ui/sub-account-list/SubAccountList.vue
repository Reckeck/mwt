<script setup lang="ts">
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { FolderInput, FolderPlus } from 'lucide-vue-next'
import { Money } from '@/entities/currency'

import { bindingBadge } from '@/entities/store'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { EmptyState } from '@/shared/ui/empty-state'
import { SettingsSection } from '@/shared/ui/settings-section'
import { storeIcon } from '@/shared/lib/store-icons'

import type { SubAccountListProps } from './sub-account-list.types'

/** A wallet's sub-accounts: each opens its own settings and can move elsewhere. */
const props = defineProps<SubAccountListProps>()
const emit = defineEmits<{ create: []; move: [store: Store] }>()
const { t } = useI18n()

</script>

<template>
  <SettingsSection :title="t('stores.subAccounts')">
    <template #actions>
      <Button size="sm" :disabled="props.archived" @click="emit('create')">
        <FolderPlus class="size-4" />
        {{ t('stores.newSubAccountShort') }}
      </Button>
    </template>
    <EmptyState v-if="!props.children.length" compact :title="t('walletSettings.noAccounts')" />
    <ul v-else class="flex flex-col divide-y divide-border">
      <li v-for="child in props.children" :key="child.store.id" class="flex items-center gap-3 py-2.5">
        <component :is="storeIcon(child.store)" class="size-4 shrink-0" :style="{ color: child.store.color }" />
        <RouterLink
          :to="{ name: 'wallet-settings', params: { id: child.store.id } }"
          class="min-w-0 flex-1 truncate text-sm font-medium hover:underline"
        >
          {{ child.store.name }}
        </RouterLink>
        <Badge v-if="bindingBadge(child.store)" variant="outline">{{ bindingBadge(child.store) }}</Badge>
        <span class="shrink-0 text-sm tabular-nums"><Money :amount="child.subtree_base" :currency="props.baseCurrency" :decimals="2" /></span>
        <Button
          variant="ghost"
          size="icon-sm"
          :title="t('stores.move')"
          :aria-label="t('stores.move')"
          @click="emit('move', child.store)"
        >
          <FolderInput class="size-4" />
        </Button>
      </li>
    </ul>
  </SettingsSection>
</template>
