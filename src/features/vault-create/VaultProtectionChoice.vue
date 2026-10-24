<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { cn } from '@/shared/lib/utils'
import { RadioGroup, RadioGroupItem } from '@/shared/ui/radio-group'

/** `true` = the file is encrypted with a password (the default and the recommended). */
const withPassword = defineModel<boolean>({ required: true })

const { t } = useI18n()

const OPTIONS = [
  { value: 'password', title: 'vaultCreate.withPassword', hint: 'vaultCreate.withPasswordHint' },
  { value: 'plain', title: 'vaultCreate.plain', hint: 'vaultCreate.plainHint' },
] as const
const choice = computed({
  get: () => (withPassword.value ? 'password' : 'plain'),
  set: (value: string) => {
    withPassword.value = value === 'password'
  },
})
</script>

<template>
  <!-- Full-New-Encrypt 1–2: the choice comes before any password is invented. -->
  <fieldset class="grid gap-1.5">
    <legend class="mb-1 text-sm font-medium">{{ t('vaultCreate.protection') }}</legend>
    <RadioGroup v-model="choice" class="grid gap-2">
      <label
        v-for="entry in OPTIONS"
        :key="entry.value"
        :for="`vault-protection-${entry.value}`"
        :class="
          cn(
            'flex cursor-pointer items-start gap-3 rounded-lg border border-border p-3 transition-colors hover:bg-accent',
            'has-[:focus-visible]:ring-2 has-[:focus-visible]:ring-ring',
            choice === entry.value && 'border-primary bg-primary/5',
          )
        "
      >
        <RadioGroupItem :id="`vault-protection-${entry.value}`" :value="entry.value" class="mt-0.5" />
        <span class="grid gap-0.5">
          <span class="text-sm font-medium">{{ t(entry.title) }}</span>
          <span class="text-xs text-muted-foreground">{{ t(entry.hint) }}</span>
        </span>
      </label>
    </RadioGroup>
  </fieldset>
</template>
