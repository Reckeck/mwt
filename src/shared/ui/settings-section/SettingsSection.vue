<script setup lang="ts">
import { Card } from '@/shared/ui/card'

import type { SettingsSectionProps } from './settings-section.types'

/**
 * One block of settings: a title, what it changes, the controls (`gap-4`
 * between fields) and an optional `#actions` slot beside the title.
 */
const props = withDefaults(defineProps<SettingsSectionProps>(), { description: undefined })
</script>

<template>
  <Card>
    <template #header>
      <div class="flex items-start justify-between gap-3">
        <div class="flex min-w-0 flex-col gap-0.5">
          <h3 class="heading-card">{{ props.title }}</h3>
          <p v-if="props.description" class="text-sm text-muted-foreground">{{ props.description }}</p>
        </div>
        <div v-if="$slots.actions" class="flex shrink-0 items-center gap-2">
          <slot name="actions" />
        </div>
      </div>
    </template>
    <div class="flex flex-col gap-4">
      <slot />
    </div>
    <template v-if="$slots.footer" #footer>
      <slot name="footer" />
    </template>
  </Card>
</template>
