<script setup lang="ts">
import { isSum, type SumPart } from '@/shared/lib/money-parts'

import { Money } from '../money'

/** A line of words and sums (`SumPart[]`): the sums are `<Money>`, so they show the other currencies. */
const props = defineProps<{ parts: SumPart[] }>()
</script>

<template>
  <template v-for="(part, at) in props.parts" :key="at">
    <Money v-if="isSum(part)" v-bind="part" />
    <template v-else>{{ part.text }}</template>
  </template>
</template>
