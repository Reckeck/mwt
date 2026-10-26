<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { ChevronDown } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { cn } from '@/shared/lib/utils'
import { Button } from '@/shared/ui/button'

/** The closed field of `StoreLeafSelect`: «Основной банк › Накопления · 1 234 $» or the hint. */
const props = defineProps<{ picked: StoreTreeNode | null; currency: string; placeholder?: string; invalid?: boolean; id?: string; open: boolean }>()

const { t } = useI18n()
</script>

<template>
  <Button
    :id="props.id"
    variant="outline"
    :class="cn('h-9 w-full justify-between gap-2 px-3 font-normal max-md:h-11', !props.picked && 'text-muted-foreground', props.invalid && 'border-destructive')"
    :aria-invalid="props.invalid || undefined"
    aria-haspopup="listbox"
    :aria-expanded="props.open"
  >
    <span class="min-w-0 truncate">
      <template v-if="props.picked">
        {{ props.picked.path.join(' › ') }} ·
        <Money :amount="props.picked.subtree_base" :currency="props.currency" :decimals="0" />
      </template>
      <template v-else>{{ props.placeholder ?? t('store.pickLeaf') }}</template>
    </span>
    <ChevronDown class="size-4 shrink-0 opacity-50" />
  </Button>
</template>
