<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Button } from '@/shared/ui/button'

/** No connected address yet (mockup 06): what connecting one does, where it starts, and the button. */
const emit = defineEmits<{ connect: [] }>()
const { t } = useI18n()
</script>

<template>
  <section class="flex flex-col items-start gap-3 rounded-xl border border-border bg-card p-6 shadow-sm" data-crypto-connect-card>
    <h2 class="text-base font-semibold">{{ t('cryptoTrack.empty.title') }}</h2>
    <p class="max-w-prose text-sm text-muted-foreground">{{ t('cryptoTrack.empty.body') }}</p>
    <p class="max-w-prose text-sm text-muted-foreground">{{ t('cryptoTrack.empty.where') }}</p>
    <Button class="max-md:h-11 max-md:w-full" data-tour="crypto-add-address" @click="emit('connect')">{{ t('cryptoTrack.empty.connect') }}</Button>
  </section>
</template>
