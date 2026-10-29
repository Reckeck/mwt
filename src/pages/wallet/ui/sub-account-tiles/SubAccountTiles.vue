<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { RouterLink } from 'vue-router'
import { Plus } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { useStoreTree } from '@/shared/api/queries'
import { Button } from '@/shared/ui/button'
import { ProgressBar } from '@/shared/ui/progress-bar'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'
import { formatPct } from '@/shared/lib/format'
import { storeIcon } from '@/shared/lib/store-icons'
import { cn } from '@/shared/lib/utils'

import { buildSubAccountTiles } from '../../lib/sub-account-tiles'

/**
 * The sub-accounts row under a wallet's header: the wallet itself and each
 * sub-account as a tile (icon and name, what it holds in its own currency,
 * ≈ the base and the share with a bar). On a phone the tiles scroll sideways.
 * A wallet with a scope to choose (not an exchange) keeps its `With sub-accounts /
 * This only` switch in the title row.
 */
const props = defineProps<{
  store: Store
  children: StoreChildRow[]
  ownBase: string
  ownShare: number
  ownPositions: StorePosition[]
  base: string
  canAddChild: boolean
  /** Show the scope switch. */
  scoped: boolean
}>()
const scope = defineModel<string>('scope', { required: true })
const emit = defineEmits<{ add: [] }>()

const { t } = useI18n()
const tree = useStoreTree()
const tiles = computed(() => buildSubAccountTiles({ ...props, tree: tree.data.value ?? [] }))
const share = (value: number) => formatPct(value * 100, { decimals: 0 })
</script>

<template>
  <section class="flex flex-col gap-2" data-tour="wallet-sub-accounts">
    <div class="flex flex-wrap items-center justify-between gap-x-3 gap-y-1">
      <h3 class="heading-card">{{ t('stores.subAccounts') }}</h3>
      <div class="flex flex-wrap items-center gap-2">
        <ToggleGroup v-if="props.scoped" v-model="scope" type="single" variant="outline" size="sm" :aria-label="t('stores.scope')">
          <ToggleGroupItem value="all" class="px-2.5">{{ t('stores.withChildren') }}</ToggleGroupItem>
          <ToggleGroupItem value="own" class="px-2.5">{{ t('stores.onlyThis') }}</ToggleGroupItem>
        </ToggleGroup>
        <Button v-if="props.canAddChild" variant="ghost" size="sm" class="text-primary" @click="emit('add')">
          <Plus class="size-4" />
          {{ t('stores.subAccountAdd') }}
        </Button>
      </div>
    </div>
    <div class="-mx-3 -my-1 flex gap-3 overflow-x-auto p-1 px-3 md:mx-0 md:my-0 md:flex-wrap md:overflow-visible md:p-0">
      <component
        :is="tile.own ? 'div' : RouterLink"
        v-for="tile in tiles"
        :key="tile.id"
        :to="tile.own ? undefined : { name: 'wallet', params: { id: tile.id } }"
        :data-entity="tile.own ? undefined : `store:${tile.id}`"
        :data-sub-tile="tile.id"
        :aria-current="tile.own ? 'page' : undefined"
        :class="
          cn(
            'flex w-40 shrink-0 flex-col gap-2 rounded-xl border bg-card p-3 shadow-sm md:min-w-44 md:flex-1 md:shrink',
            tile.own ? 'border-primary ring-1 ring-primary' : 'border-border transition-colors hover:bg-accent/40',
            tile.store.archived && 'opacity-60',
          )
        "
      >
        <span class="flex min-w-0 items-center gap-2">
          <span class="flex size-9 shrink-0 items-center justify-center rounded-[10px] bg-muted">
            <component :is="storeIcon(tile.store)" class="store-tint size-4" :style="{ '--store-color': tile.store.color }" />
          </span>
          <span class="min-w-0 truncate text-sm font-medium">{{ tile.own ? t('stores.ownFunds') : tile.store.name }}</span>
        </span>
        <span class="flex items-baseline gap-1 text-xl font-semibold tabular-nums">
          <Money :amount="tile.amount" :currency="tile.currency" :decimals="2" />
          <span v-if="tile.extra" class="text-xs font-normal text-muted-foreground">+{{ tile.extra }}</span>
        </span>
        <span class="flex items-center justify-between gap-2 text-xs text-muted-foreground tabular-nums">
          <span class="min-w-0 truncate">≈ <Money :amount="tile.baseAmount" :currency="props.base" :decimals="0" /></span>
          <span class="shrink-0">{{ share(tile.share) }}</span>
        </span>
        <ProgressBar :value="tile.share * 100" size="sm" />
      </component>
    </div>
  </section>
</template>
