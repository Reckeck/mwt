<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Check } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { cn } from '@/shared/lib/utils'
import { Button } from '@/shared/ui/button'

import type { LeafGroup } from './store-leaf-select.lib'

/** The rows of `StoreLeafSelect`: a heading per group, a wallet per row with its balance and a tick. */
const props = defineProps<{ groups: LeafGroup[]; picked: string; currency: string }>()
const emit = defineEmits<{ pick: [id: string] }>()

const { t } = useI18n()
</script>

<template>
  <div role="listbox" class="grid gap-1">
    <p v-if="!props.groups.length" class="px-3 py-4 text-sm text-muted-foreground">{{ t('store.noWallets') }}</p>
    <section v-for="group in props.groups" :key="group.key" class="grid gap-0.5">
      <p v-if="group.heading" class="px-3 pt-2 text-xs font-medium text-muted-foreground">{{ group.heading }}</p>
      <Button
        v-for="leaf in group.leaves"
        :key="leaf.store.id"
        variant="ghost"
        role="option"
        :aria-selected="leaf.store.id === props.picked"
        :class="cn('h-10 justify-between gap-2 px-3 font-normal max-md:h-11', group.heading && 'pl-5')"
        @click="emit('pick', leaf.store.id)"
      >
        <span class="min-w-0 truncate">{{ leaf.store.name }}</span>
        <span class="flex shrink-0 items-center gap-2 text-muted-foreground">
          <Money :amount="leaf.subtree_base" :currency="props.currency" :decimals="0" />
          <Check class="size-4" :class="leaf.store.id === props.picked ? 'text-primary' : 'invisible'" aria-hidden="true" />
        </span>
      </Button>
    </section>
  </div>
</template>
