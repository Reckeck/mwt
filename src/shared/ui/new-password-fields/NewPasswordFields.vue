<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Eye, EyeOff } from 'lucide-vue-next'

import { MIN_VAULT_PASSWORD } from '@/shared/lib/vault-password'
import { Button } from '@/shared/ui/button'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'

import type { NewPasswordFieldsProps } from './new-password-fields.types'

/**
 * A new password typed twice (Full-New-Encrypt 1, 5): the same pair in `New vault`
 * and `Set password`. Shown as typed on request: a new password is easy to mistype twice alike.
 */
const props = withDefaults(defineProps<NewPasswordFieldsProps>(), { label: undefined })
const password = defineModel<string>('password', { required: true })
const repeat = defineModel<string>('repeat', { required: true })

const { t } = useI18n()
const reveal = ref(false)
const match = computed(() => repeat.value.length > 0 && repeat.value === password.value)
const type = computed(() => (reveal.value ? 'text' : 'password'))
</script>

<template>
  <FormField
    :for="`${props.id}-new`"
    :label="props.label ?? t('auth.password')"
    :hint="t('vaultCreate.passwordHint', { count: MIN_VAULT_PASSWORD })"
    required
    :error="props.error"
  >
    <div class="flex gap-1.5">
      <Input :id="`${props.id}-new`" v-model="password" :type="type" autocomplete="new-password" />
      <Button
        variant="outline"
        size="icon"
        type="button"
        :aria-pressed="reveal"
        :aria-label="reveal ? t('auth.hidePassword') : t('auth.showPassword')"
        :title="reveal ? t('auth.hidePassword') : t('auth.showPassword')"
        @click="reveal = !reveal"
      >
        <component :is="reveal ? EyeOff : Eye" class="size-4" />
      </Button>
    </div>
  </FormField>

  <FormField :for="`${props.id}-repeat`" :label="t('auth.passwordRepeat')" required :error="props.repeatError">
    <Input :id="`${props.id}-repeat`" v-model="repeat" :type="type" autocomplete="new-password" />
    <p v-if="match && !props.repeatError" class="text-xs text-success">✓ {{ t('auth.passwordMatch') }}</p>
  </FormField>
</template>
