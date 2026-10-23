<script setup lang="ts">
import { Label } from '@/shared/ui/label'
import { Switch } from '@/shared/ui/switch'

import type { SwitchRowProps } from './switch-row.types'

/** An on/off setting: its name (and what it does) on the left, the switch on the right. */
const props = withDefaults(defineProps<SwitchRowProps>(), { hint: undefined, disabled: false, plain: false })
const model = defineModel<boolean>({ required: true })
</script>

<template>
  <div :class="['flex items-center justify-between gap-3', !props.plain && 'rounded-md border border-border px-3 py-2']">
    <div class="grid min-w-0 gap-0.5">
      <Label :for="props.id">{{ props.label }}</Label>
      <p v-if="props.hint" class="text-xs text-muted-foreground">{{ props.hint }}</p>
    </div>
    <Switch :id="props.id" v-model="model" :disabled="props.disabled" />
  </div>
</template>
