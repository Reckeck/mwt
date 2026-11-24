<script setup lang="ts">
import { computed } from 'vue'

import { SIGN_TONE_CLASS, type SignTone, signTone } from '@/shared/lib/format'
import { cn } from '@/shared/lib/utils'

import type { DeltaProps } from './delta.types'

/** A change against a base: the sign as text (never colour alone), the colour by direction. */
const props = withDefaults(defineProps<DeltaProps>(), { text: undefined, inverse: false })

const direction = computed(() => signTone(props.value))
const SIGN_MARK: Record<SignTone, string> = { positive: '+', negative: '−', neutral: '' }
const tone = computed<SignTone>(() => {
  if (!props.inverse || direction.value === 'neutral') return direction.value
  return direction.value === 'positive' ? 'negative' : 'positive'
})
const body = computed(() => props.text ?? String(Math.abs(Number(props.value))))
</script>

<template>
  <span :class="cn('tabular-nums', SIGN_TONE_CLASS[tone])">{{ SIGN_MARK[direction] }}{{ body }}</span>
</template>
