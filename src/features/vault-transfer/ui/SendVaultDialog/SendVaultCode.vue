<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { required, useFormValidation } from '@/shared/lib/form'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'

/** Step two: the six digits shown on the receiving device. */
const props = defineProps<{ view: TransferSendView }>()
const emit = defineEmits<{ submit: [code: string] }>()
const { t } = useI18n()

const code = ref('')
const wrong = ref(false)

// A wrong code comes back from the other device: clear the field for the next try.
watch(
  () => [props.view.wrong_code, props.view.attempts_left] as const,
  ([isWrong]) => {
    wrong.value = isWrong
    if (!isWrong) return
    // An empty field after a wrong code says why, not «fill this in».
    code.value = ''
    form.reset()
  },
)
// Typing a new code clears the note; the field emptied by a wrong code does not.
watch(code, (value) => {
  if (value) wrong.value = false
})

const form = useFormValidation(() => ({
  code:
    required(code.value) ??
    (/^\d{6}$/.test(code.value.replace(/\s/g, '')) ? null : t('transfer.send.codeFormat')),
}))

function submit(event: Event) {
  form.submit(() => emit('submit', code.value.replace(/\s/g, '')), event)
}
</script>

<template>
  <form id="transfer-code-form" class="grid gap-3" @submit.prevent="submit">
    <p class="text-sm text-muted-foreground">{{ t('transfer.send.codeHint', { name: view.device_name }) }}</p>
    <FormField for="transfer-code" :label="t('transfer.send.code')" required :error="form.error('code') ?? (wrong ? t('transfer.send.codeWrong', { count: view.attempts_left }) : undefined)">
      <Input
        id="transfer-code"
        v-model="code"
        inputmode="numeric"
        autocomplete="one-time-code"
        maxlength="7"
        class="font-mono text-lg tracking-widest"
      />
    </FormField>
  </form>
</template>
