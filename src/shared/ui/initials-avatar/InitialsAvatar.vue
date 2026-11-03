<script setup lang="ts">
import { computed } from 'vue'

import { cn } from '@/shared/lib/utils'

import { initialsOf } from './initials-avatar.constants'
import type { InitialsAvatarProps } from './initials-avatar.types'

/** A round tile with the initials of a name: the stand-in for a photo in lists and card headers. */
const props = withDefaults(defineProps<InitialsAvatarProps>(), { size: 'md' })

const SIZE_CLASS = { sm: 'size-8 text-[0.6875rem]', md: 'size-9 text-xs', lg: 'size-12 text-base' } as const
const text = computed(() => initialsOf(props.name))
</script>

<template>
  <span
    :class="cn('inline-flex shrink-0 select-none items-center justify-center rounded-full bg-secondary font-semibold text-secondary-foreground', SIZE_CLASS[props.size])"
    aria-hidden="true"
  >
    {{ text }}
  </span>
</template>
