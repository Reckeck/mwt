<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { FolderPlus } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { StoreRowLink } from '@/entities/store'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { formatPct } from '@/shared/lib/format'
import { storeIcon } from '@/shared/lib/store-icons'

/** Money kept on the wallet itself next to what each sub-account holds, with their shares. */
const props = defineProps<{
  store: Store
  children: StoreChildRow[]
  ownBase: string
  ownShare: number
  currency: string
  canAddChild: boolean
}>()
const emit = defineEmits<{ add: [] }>()

const { t } = useI18n()
const share = (value: number) => formatPct(value * 100, { decimals: 0 })
</script>

<template>
  <Card>
    <template #header>
      <div class="flex items-center justify-between gap-2">
        <h3 class="heading-card">{{ t('stores.subAccounts') }}</h3>
        <Button v-if="props.canAddChild" variant="ghost" size="sm" @click="emit('add')">
          <FolderPlus class="size-4" />
          {{ t('stores.newSubAccountShort') }}
        </Button>
      </div>
    </template>
    <div class="flex flex-col divide-y divide-border">
      <div class="flex items-center gap-3 py-2.5">
        <component :is="storeIcon(props.store)" class="store-tint size-4 shrink-0" :style="{ '--store-color': props.store.color }" />
        <span class="min-w-0 flex-1 truncate text-sm font-medium">{{ t('stores.ownFunds') }}</span>
        <span class="shrink-0 text-xs tabular-nums text-muted-foreground">{{ share(props.ownShare) }}</span>
        <Money class="shrink-0 text-sm" :amount="props.ownBase" :currency="props.currency" :decimals="2" />
      </div>
      <StoreRowLink
        v-for="child in props.children"
        :key="child.store.id"
        class="py-2.5"
        :store="child.store"
        :amount="child.subtree_base"
        :currency="props.currency"
        :share="child.share"
        :decimals="2"
      />
    </div>
  </Card>
</template>
