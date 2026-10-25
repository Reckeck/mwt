<script setup lang="ts">
import { RouterLink, type RouteLocationRaw } from 'vue-router'

import { pageEntityKey } from '@/shared/lib/entity-link'
import { quickAddOpen } from '@/shared/lib/quick-add'
import { Button, buttonVariants } from '@/shared/ui/button'

/** The one button of a thing to do: opens the quick add, or goes where it is done. */
const props = defineProps<{ to: RouteLocationRaw | 'quick-add'; label: string }>()
</script>

<template>
  <Button v-if="props.to === 'quick-add'" variant="outline" size="sm" class="max-md:h-10" @click="quickAddOpen = true">
    {{ props.label }}
  </Button>
  <RouterLink v-else :to="props.to" :data-entity="pageEntityKey(props.to)" :class="buttonVariants({ variant: 'outline', size: 'sm' })" class="max-md:h-10">
    {{ props.label }}
  </RouterLink>
</template>
