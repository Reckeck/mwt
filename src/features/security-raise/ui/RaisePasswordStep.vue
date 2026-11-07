<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { CircleAlert } from 'lucide-vue-next'

import { passwordStrength, STRENGTH_PERCENT } from '@/shared/lib/vault-password'
import { Alert, AlertDescription, AlertTitle } from '@/shared/ui/alert'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { ProgressBar } from '@/shared/ui/progress-bar'

import type { RaiseFieldErrors } from '../security-raise.types'

/**
 * Step 2: the current PIN (level 2 only, a plain vault has none) and the new password typed twice, with a
 * strength meter that is always in place so the form does not jump while typing.
 */
const props = defineProps<{ level: SecurityLevel; restarted: boolean; errors: RaiseFieldErrors }>()
const pin = defineModel<string>('pin', { required: true })
const password = defineModel<string>('password', { required: true })
const repeat = defineModel<string>('repeat', { required: true })

const { t } = useI18n()
const strength = computed(() => passwordStrength(password.value).strength)
const TONE = { weak: 'destructive', medium: 'warning', strong: 'primary' } as const
</script>

<template>
  <div class="grid gap-4">
    <Alert v-if="props.restarted">
      <CircleAlert />
      <AlertTitle>{{ t('securityRaise.restart.title') }}</AlertTitle>
      <AlertDescription>{{ t('securityRaise.restart.text') }}</AlertDescription>
    </Alert>
    <FormField v-if="props.level === 'pin'" for="raise-pin" :label="t('securityRaise.password.currentPin')" required :error="props.errors.pin">
      <Input
        id="raise-pin"
        v-model="pin"
        type="password"
        autocomplete="current-password"
        :placeholder="t('securityRaise.password.currentPinPlaceholder')"
      />
    </FormField>
    <FormField
      for="raise-password"
      :label="t('securityRaise.password.new')"
      required
      :error="props.errors.password"
      :hint="t('securityRaise.password.newHint')"
    >
      <Input id="raise-password" v-model="password" type="password" autocomplete="new-password" />
    </FormField>
    <!-- Always drawn: an empty meter keeps the height. -->
    <div class="grid gap-1.5" data-raise-strength>
      <div class="flex items-center justify-between gap-2 text-xs">
        <span class="text-muted-foreground">{{ t('securityRaise.password.strength') }}</span>
        <span class="font-medium">
          {{ password ? t(`securityRaise.password.${strength}`) : t('securityRaise.password.strengthEmpty') }}
        </span>
      </div>
      <ProgressBar
        :value="password ? STRENGTH_PERCENT[strength] : 0"
        :tone="TONE[strength]"
        size="sm"
        :label="t('securityRaise.password.strength')"
      />
    </div>
    <FormField for="raise-repeat" :label="t('securityRaise.password.repeat')" required :error="props.errors.repeat">
      <Input id="raise-repeat" v-model="repeat" type="password" autocomplete="new-password" />
    </FormField>
  </div>
</template>
