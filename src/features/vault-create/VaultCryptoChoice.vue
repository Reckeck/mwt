<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { cn } from '@/shared/lib/utils'
import { RadioGroup, RadioGroupItem } from '@/shared/ui/radio-group'

/** `true` = the vault keeps crypto wallets, exchanges and coins. */
const crypto = defineModel<boolean>({ required: true })

const { t } = useI18n()

const OPTIONS = ['no', 'yes'] as const
const choice = computed({
  get: () => (crypto.value ? 'yes' : 'no'),
  set: (value: string) => {
    crypto.value = value === 'yes'
  },
})
</script>

<template>
  <!-- Asked once per vault; Settings → Хранилище → Направление changes it later. -->
  <fieldset class="grid gap-1.5">
    <legend class="mb-1 text-sm font-medium">{{ t('auth.crypto.title') }}</legend>
    <RadioGroup v-model="choice" class="grid gap-2 sm:grid-cols-2">
      <label
        v-for="entry in OPTIONS"
        :key="entry"
        :for="`vault-crypto-${entry}`"
        :class="
          cn(
            'flex cursor-pointer items-start gap-3 rounded-lg border border-border p-3 transition-colors hover:bg-accent',
            'has-[:focus-visible]:ring-2 has-[:focus-visible]:ring-ring',
            choice === entry && 'border-primary bg-primary/5',
          )
        "
      >
        <RadioGroupItem :id="`vault-crypto-${entry}`" :value="entry" class="mt-0.5" />
        <span class="grid gap-0.5">
          <span class="text-sm font-medium">{{ t(`auth.crypto.${entry}`) }}</span>
          <span class="text-xs text-muted-foreground">{{ t(`auth.crypto.${entry}Hint`) }}</span>
        </span>
      </label>
    </RadioGroup>
    <p class="text-xs text-muted-foreground">{{ t('auth.crypto.later') }}</p>
  </fieldset>
</template>
