<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { ChevronDown, ChevronRight } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'

/** The chevron beside a wallet that has sub-accounts: unfolds or folds them. */
const props = defineProps<{ name: string; open: boolean }>()
const emit = defineEmits<{ toggle: [] }>()
const { t } = useI18n()
</script>

<template>
  <Button
    variant="ghost"
    size="icon-sm"
    class="relative z-10 text-muted-foreground"
    :aria-expanded="props.open"
    :aria-label="t(props.open ? 'stores.table.collapse' : 'stores.table.expand', { name: props.name })"
    data-wallet-expand
    @click="emit('toggle')"
  >
    <component :is="props.open ? ChevronDown : ChevronRight" class="size-4" />
  </Button>
</template>
