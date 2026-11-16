<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { formatDay } from '@/shared/lib/format'
import { Button } from '@/shared/ui/button'
import { SectionState } from '@/shared/ui/section-state'
import { SettingsSection } from '@/shared/ui/settings-section'

import { useNetworkSettings, useSetConsent, useTrackingConsent } from '../model/use-crypto-network'

/**
 * Settings · Crypto · Consent: the one-time consent to read the followed addresses from the networks, and its
 * withdrawal. Without it the module makes no network call at all; withdrawing stops them (a running pass ends
 * before its next request) and deletes nothing.
 */
const consent = useTrackingConsent()
const { grant, withdraw } = useSetConsent()
const settings = useNetworkSettings()
const { t } = useI18n()
const given = computed(() => consent.data.value === true)
const stateText = computed(() => {
  if (consent.data.value === true) {
    const at = settings.data.value?.consent_at
    const device = settings.data.value?.consent_device
    if (!at) return t('cryptoTrack.consent.stateGiven')
    const day = formatDay(at)
    return device ? t('cryptoTrack.consent.givenOnDevice', { date: day, device }) : t('cryptoTrack.consent.givenOn', { date: day })
  }
  return consent.data.value === false ? t('cryptoTrack.consent.stateWithdrawn') : t('cryptoTrack.consent.stateNone')
})
</script>

<template>
  <SectionState :loading="consent.isPending.value" :error="consent.error.value" @retry="consent.refetch()">
    <div class="flex flex-col gap-4">
      <SettingsSection :title="t('cryptoTrack.consent.title')">
        <div class="flex items-start justify-between gap-3" data-tour="crypto-consent">
          <div class="flex min-w-0 flex-col">
            <span class="font-medium">{{ t('cryptoTrack.consent.what') }}</span>
            <span class="text-sm text-muted-foreground">{{ stateText }}</span>
          </div>
          <div class="flex shrink-0">
            <Button
              :variant="given ? 'ghost' : 'outline'"
              :class="given ? 'min-w-28 text-destructive' : 'min-w-28'"
              :disabled="withdraw.isPending.value || grant.isPending.value"
              @click="given ? withdraw.mutate() : grant.mutate()"
            >{{ given ? t('cryptoTrack.consent.withdraw') : t('cryptoTrack.consent.grant') }}</Button>
          </div>
        </div>
        <p class="text-sm">{{ given ? t('cryptoTrack.consent.effectOn') : t('cryptoTrack.consent.effectOff') }}</p>
      </SettingsSection>
      <SettingsSection :title="t('cryptoTrack.consent.sendsTitle')">
        <ul class="flex flex-col gap-2 text-sm">
          <li>{{ t('cryptoTrack.consent.sendsAddresses') }}</li>
          <li>{{ t('cryptoTrack.consent.sendsTokens') }}</li>
          <li>{{ t('cryptoTrack.consent.sendsNever') }}</li>
        </ul>
      </SettingsSection>
      <div class="flex flex-col gap-1 rounded-lg bg-muted p-4 text-sm">
        <h3 class="font-medium text-muted-foreground">{{ t('cryptoTrack.consent.keepsTitle') }}</h3>
        <p>{{ t('cryptoTrack.consent.keepsBody') }}</p>
      </div>
    </div>
  </SectionState>
</template>
