<script setup lang="ts">
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { Badge } from '@/shared/ui/badge'
import { formatPct } from '@/shared/lib/format'
import { storeIcon } from '@/shared/lib/store-icons'
import { cn } from '@/shared/lib/utils'

import { bindingBadge } from '../../model/wallet-catalog'
import type { StoreRowLinkProps } from './store-row-link.types'

/** One sub-account as a row that opens it: icon, name, binding, share and value. */
const props = withDefaults(defineProps<StoreRowLinkProps>(), { depth: 1, share: null, decimals: undefined })
const { t } = useI18n()
</script>

<template>
  <RouterLink
    :to="{ name: 'wallet', params: { id: props.store.id } }"
    :data-entity="`store:${props.store.id}`"
    :class="
      cn(
        '-mx-2 flex items-center gap-2 rounded-md px-2 py-1.5 text-sm transition-colors hover:bg-accent',
        props.depth > 1 && 'pl-7',
        props.store.archived && 'opacity-60',
      )
    "
  >
    <component :is="storeIcon(props.store)" class="store-tint size-4 shrink-0" :style="{ '--store-color': props.store.color }" />
    <span class="min-w-0 flex-1 truncate">{{ props.store.name }}</span>
    <Badge v-if="props.store.archived" variant="outline">{{ t('stores.archived') }}</Badge>
    <Badge v-else-if="bindingBadge(props.store)" variant="outline">{{ bindingBadge(props.store) }}</Badge>
    <span v-if="props.share !== null" class="shrink-0 text-xs tabular-nums text-muted-foreground">
      {{ formatPct(props.share * 100, { decimals: 0 }) }}
    </span>
    <Money class="shrink-0 tabular-nums" :amount="props.amount" :currency="props.currency" :decimals="props.decimals" />
  </RouterLink>
</template>
