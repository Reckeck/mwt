<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { toast } from 'vue-sonner'

import { ExchangeKeyForm } from '@/features/exchange-key'
import { useWalletCatalog } from '@/entities/store'
import { Label } from '@/shared/ui/label'
import { Select } from '@/shared/ui/select'
import { call, parseError } from '@/shared/api/ipc'
import { cn } from '@/shared/lib/utils'
import { codeErrorMessage } from '@/shared/lib/vault-error'

import { updateInputOf, useStoreUpdate } from '../model/use-store-update'
import type { StoreFormProps } from '../store-edit.types'

const props = defineProps<StoreFormProps>()
const { t } = useI18n()
const queryClient = useQueryClient()
const catalog = useWalletCatalog()

/** Import intervals offered, in seconds; 0 = off. */
const INTERVALS = [0, 300, 900, 3600, 21_600, 86_400]

const provider = computed(() => props.store.exchange?.provider ?? null)
const interval = ref('900')
watch(
  () => props.store.exchange?.import_interval_sec,
  (value) => {
    interval.value = String(value ?? 900)
  },
  { immediate: true },
)

const save = useStoreUpdate()

function pick(value: ExchangeProvider) {
  if (value === provider.value) return
  save.mutate(updateInputOf(props.store, { provider: value }))
}

const setInterval = useMutation({
  mutationFn: (seconds: number) =>
    call('set_import_interval', { input: { store_id: props.store.id, interval_sec: seconds } }),
  onSuccess: () => {
    toast.success(t('stores.saved'))
    void queryClient.invalidateQueries({ queryKey: ['store'] })
  },
  onError: (err) => toast.error(t('stores.saveFailed'), { description: codeErrorMessage(parseError(err), t) }),
})

function intervalLabel(seconds: number): string {
  if (seconds === 0) return t('exchange.importOff')
  if (seconds < 3600) return t('exchange.everyMinutes', { count: seconds / 60 })
  return t('exchange.everyHours', { count: seconds / 3600 })
}
</script>

<template>
  <div class="@container grid gap-4">
    <div class="grid gap-2">
      <Label>{{ t('provider.title') }}</Label>
      <div class="grid gap-2 @md:grid-cols-3">
        <button
          v-for="info in catalog.data.value?.providers ?? []"
          :key="info.provider"
          type="button"
          :disabled="!info.supported || save.isPending.value"
          :aria-pressed="provider === info.provider"
          :class="
            cn(
              'flex items-center justify-between rounded-lg border border-border p-3 text-left transition-colors hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50',
              provider === info.provider && 'border-primary bg-primary/5',
            )
          "
          @click="pick(info.provider)"
        >
          <span class="text-sm font-medium">{{ t(`provider.${info.provider}`) }}</span>
          <span v-if="!info.supported" class="text-xs text-muted-foreground">{{ t('provider.soon') }}</span>
        </button>
      </div>
      <p v-if="!provider" class="text-sm text-muted-foreground">{{ t('provider.pickFirst') }}</p>
    </div>

    <!-- Keys only once the exchange is known: they mean nothing without it. -->
    <template v-if="provider">
      <ExchangeKeyForm :store-id="props.store.id" />
      <div class="grid max-w-xs gap-1.5">
        <Label for="exchange-interval">{{ t('exchange.importInterval') }}</Label>
        <Select
          id="exchange-interval"
          :model-value="interval"
          @update:model-value="(value) => setInterval.mutate(Number(value))"
        >
          <option v-for="seconds in INTERVALS" :key="seconds" :value="String(seconds)">{{ intervalLabel(seconds) }}</option>
        </Select>
      </div>
    </template>
  </div>
</template>
