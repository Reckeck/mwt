<script setup lang="ts">
import { FixedCosts } from '@/widgets/fixed-costs'
import { UtilitiesDue } from '@/widgets/utilities-due'
import { UtilityStats } from '@/widgets/utility-stats'
import { useWidgetSize } from '@/shared/ui/widget-frame'

/**
 * «ЖКХ и аренда» (plan H2: three utility widgets in one): S — what is due this
 * month; M — the fixed costs and their changes too; L — a year of bills by item.
 */
const size = useWidgetSize()
</script>

<template>
  <div class="flex flex-col gap-3">
    <UtilitiesDue />
    <FixedCosts v-if="size !== 's'" />
    <UtilityStats v-if="size === 'l'" />
  </div>
</template>
