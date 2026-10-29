<script setup lang="ts">
import { TapHint } from '@/shared/ui/tap-hint'

import type { ReportTermProps } from './report-term.types'

/** A column or figure name with its meaning (hover with a mouse, tap on touch): see `TapHint`. */
const props = defineProps<ReportTermProps>()
</script>

<template>
  <TapHint :label="props.label" :hint="props.hint" />
</template>
