<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { required, useFormValidation } from '@/shared/lib/form'
import { useSettingsStore } from '@/shared/store/settings'
import { Button } from '@/shared/ui/button'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { SettingsSection } from '@/shared/ui/settings-section'
import { SwitchRow } from '@/shared/ui/switch-row'

import { AUTOLOCK_MAX, AUTOLOCK_MIN, DEFAULT_AUTOLOCK, minutesError } from '../lib/autolock'
import { useHasDeviceKeys } from '../model/use-has-device-keys'

/**
 * `Storage` > autolock (Security 1): lock after idle minutes, off only while the
 * device holds no exchange keys. Sleep and the screen lock close the vault
 * whatever is set here, and the card says so.
 */
const { t } = useI18n()
const settings = useSettingsStore()
const hasKeys = useHasDeviceKeys()

const saved = computed<AutolockSettings>(() => settings.app?.autolock ?? DEFAULT_AUTOLOCK)
const enabled = ref(saved.value.enabled)
const minutes = ref(String(saved.value.minutes))
watch(saved, (value) => {
  enabled.value = value.enabled
  minutes.value = String(value.minutes)
})
// Off keeps the field's place and its last saved value; a half-typed one is dropped.
watch(enabled, (on) => {
  if (!on) minutes.value = String(saved.value.minutes)
})

const form = useFormValidation(() => ({ minutes: required(minutes.value) ?? minutesError(minutes.value) }))

const save = useMutation({
  mutationFn: () =>
    call<AutolockSettings>('set_autolock', { input: { enabled: enabled.value, minutes: Number(minutes.value) } }),
  onSuccess: (data) => {
    if (settings.app) settings.app = { ...settings.app, autolock: data }
  },
  meta: { success: 'autolock.saved', error: 'autolock.saveFailed' },
})
</script>

<template>
  <SettingsSection :title="t('autolock.title')">
    <form class="flex flex-col gap-4" novalidate @submit.prevent="form.submit(save.mutate, $event)">
      <SwitchRow
        v-model="enabled"
        plain
        id="autolock-enabled"
        :label="t('autolock.enabled')"
        :hint="hasKeys ? t('autolock.keysHint') : t('autolock.noKeysHint')"
        :disabled="hasKeys"
      />
      <FormField
        for="autolock-minutes"
        :label="t('autolock.minutes')"
        required
        :error="form.error('minutes')"
        :hint="t('autolock.range', { min: AUTOLOCK_MIN, max: AUTOLOCK_MAX })"
      >
        <div class="relative">
          <Input
            id="autolock-minutes"
            v-model="minutes"
            inputmode="numeric"
            class="pr-14"
            :disabled="!enabled"
          />
          <span class="pointer-events-none absolute inset-y-0 right-3 flex items-center text-sm text-muted-foreground">
            {{ t('autolock.unit') }}
          </span>
        </div>
      </FormField>
      <p class="text-sm text-muted-foreground">{{ t('autolock.always') }}</p>
      <div>
        <Button type="submit" class="max-md:h-11 max-md:w-full" :disabled="save.isPending.value">
          {{ t('autolock.save') }}
        </Button>
      </div>
    </form>
  </SettingsSection>
</template>
