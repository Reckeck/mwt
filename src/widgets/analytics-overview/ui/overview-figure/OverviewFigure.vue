<script setup lang="ts">
import { ReportTerm } from '@/entities/report'

import type { OverviewFigureProps } from './overview-figure.types'

/** A labelled figure inside an overview card; the value goes in the slot. */
const props = defineProps<OverviewFigureProps>()
</script>

<template>
  <div class="grid min-w-0 gap-0.5">
    <p class="truncate text-xs text-muted-foreground"><ReportTerm :label="props.label" :hint="props.hint" /></p>
    <p class="figure truncate"><slot /></p>
  </div>
</template>
