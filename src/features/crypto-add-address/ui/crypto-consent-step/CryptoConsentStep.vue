<script setup lang="ts">
import { useI18n } from 'vue-i18n'

/** The one-time consent (mockup 08): who sees the address, that it is read-only, and where the data goes. */
const { t } = useI18n()
</script>

<template>
  <div class="flex flex-col gap-3 text-sm">
    <h3 class="font-semibold">{{ t('cryptoTrack.connect.willHappen') }}</h3>
    <ul class="flex flex-col gap-2">
      <li>{{ t('cryptoTrack.connect.consentNetwork') }}</li>
      <li>{{ t('cryptoTrack.connect.consentReadOnly') }}</li>
      <li>{{ t('cryptoTrack.connect.consentStorage') }}</li>
    </ul>
    <div class="rounded-lg bg-muted p-3">
      <p class="font-semibold">{{ t('cryptoTrack.connect.noSecretsTitle') }}</p>
      <p class="mt-1 text-muted-foreground">{{ t('cryptoTrack.connect.noSecretsBody') }}</p>
    </div>
  </div>
</template>
