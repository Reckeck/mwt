<script setup lang="ts">
import { ArrowLeft } from 'lucide-vue-next'
import { useRouter } from 'vue-router'

import { Button } from '@/shared/ui/button'

import type { BackChipProps } from './back-chip.types'
import { useBackKeys } from './use-back-keys'

/**
 * `← Spending · October` (Paths 1 · 5): the way back to the screen a list was
 * opened from. Esc and Alt ← do the same while nothing floats over the page.
 */
const props = defineProps<BackChipProps>()
const router = useRouter()

function back() {
  void router.push(props.to)
}

useBackKeys(back)
</script>

<template>
  <Button variant="secondary" size="sm" class="rounded-full" data-back-chip @click="back">
    <ArrowLeft class="size-3.5" aria-hidden="true" />
    {{ props.label }}
  </Button>
</template>
