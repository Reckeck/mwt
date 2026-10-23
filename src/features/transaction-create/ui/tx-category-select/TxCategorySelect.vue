<script setup lang="ts">
import { CategorySelect, type CategorySelectProps } from '@/features/category-select'

/**
 * The category picker of the operation forms: the one place this slice reaches
 * into `category-select`, so the full and the short forms share it.
 */
const props = defineProps<CategorySelectProps>()
const model = defineModel<string | string[]>({ required: true })
</script>

<template>
  <CategorySelect v-bind="props" v-model="model" />
</template>
