<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Plus } from 'lucide-vue-next'

import { currencySymbol } from '@/shared/lib/format'
import { Button } from '@/shared/ui/button'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'

import type { WalletMode } from '../stores.types'

/**
 * `In wallets 40,605.10 $ · rates for today 14:20` and the switch beside it:
 * the base, the budget's currency, per currency (Currency-1 · 2). The total
 * itself comes in the slot: it is a glance, which this page owns.
 */
const props = defineProps<{ phone: boolean; base: string; second: string; rates: string }>()
const emit = defineEmits<{ create: [] }>()
const mode = defineModel<WalletMode>('mode', { required: true })
const { t } = useI18n()

const modes = computed(() =>
  [
    { value: 'base' as const, label: currencySymbol(props.base) },
    ...(props.second ? [{ value: 'second' as const, label: currencySymbol(props.second) }] : []),
    ...(props.phone ? [] : [{ value: 'assets' as const, label: t('stores.view.assets') }]),
  ],
)
/** A phone has no `By currency`: the table view falls back to the base. */
const picked = computed({
  get: () => (modes.value.some((entry) => entry.value === mode.value) ? mode.value : 'base'),
  set: (value: WalletMode) => {
    if (value) mode.value = value
  },
})
</script>

<template>
  <div class="flex flex-wrap items-end justify-between gap-x-4 gap-y-3" data-tour="stores-total">
    <div class="flex min-w-0 flex-col gap-0.5">
      <span class="text-sm text-muted-foreground">{{ t('stores.total') }}</span>
      <slot />
      <span v-if="props.rates && !props.phone" class="text-xs text-muted-foreground">{{ props.rates }}</span>
    </div>
    <div class="flex items-center gap-2">
      <ToggleGroup
        v-if="modes.length > 1"
        v-model="picked"
        type="single"
        size="sm"
        class="rounded-[10px] bg-muted p-0.75"
        :aria-label="t('stores.view.label')"
        data-tour="stores-view"
      >
        <ToggleGroupItem
          v-for="entry in modes"
          :key="entry.value"
          :value="entry.value"
          class="h-6 rounded-lg px-2.5 text-xs touch-hit-lg pointer-coarse:min-w-10 data-[state=on]:bg-background data-[state=on]:shadow-sm"
        >
          {{ entry.label }}
        </ToggleGroupItem>
      </ToggleGroup>
      <Button
        variant="outline"
        :size="props.phone ? 'icon' : 'default'"
        :aria-label="t('stores.createShort')"
        data-tour="stores-create"
        @click="emit('create')"
      >
        <Plus class="size-4" />
        <span v-if="!props.phone">{{ t('stores.createShort') }}</span>
      </Button>
    </div>
  </div>
</template>
