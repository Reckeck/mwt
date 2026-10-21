<script setup lang="ts">
import { watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { Button } from '@/shared/ui/button'
import { useFormValidation } from '@/shared/lib/form'
import { call } from '@/shared/api/ipc'

import { invalidateLoyalty } from './model/invalidate'
import { useLoyaltyInfo } from './model/use-loyalty-info'
import LoyaltyInfoFields from './ui/LoyaltyInfoFields.vue'

/** A bonus programme: what its points are called, what one is worth, when some burn. */
const props = defineProps<{ store: Store }>()
const { t } = useI18n()
const queryClient = useQueryClient()

const state = useLoyaltyInfo()
const form = useFormValidation(state.rules, { resetOn: () => props.store.id })
watch(() => props.store, state.load, { immediate: true })

const save = useMutation({
  mutationFn: () =>
    call<Store>('set_loyalty_info', {
      input: { store_id: props.store.id, info: state.toInfo() } satisfies SetLoyaltyInfoInput,
    }),
  onSuccess: () => invalidateLoyalty(queryClient),
  meta: { error: 'settings.saveFailed', success: 'settings.saved' },
})
</script>

<template>
  <form class="grid gap-4" @submit.prevent="form.submit(save.mutate, $event)">
    <LoyaltyInfoFields :state="state" :error="form.error" />
    <div>
      <Button type="submit" :disabled="save.isPending.value">{{ t('actions.save') }}</Button>
    </div>
  </form>
</template>
