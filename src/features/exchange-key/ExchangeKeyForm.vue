<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'

import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { call, parseError } from '@/shared/api/ipc'

import type { ExchangeKeyFormProps } from './exchange-key.types'
import { useExchangeKeyStatus } from './useExchangeKeyStatus'

const props = defineProps<ExchangeKeyFormProps>()

const { t } = useI18n()

const { status, hasKey, hint } = useExchangeKeyStatus(
  computed(() => props.storeId),
  computed(() => true),
)

const apiKey = ref('')
const apiSecret = ref('')
const busy = ref(false)

async function save() {
  busy.value = true
  try {
    await call('set_exchange_key', {
      input: { store_id: props.storeId, api_key: apiKey.value, api_secret: apiSecret.value },
    })
    apiKey.value = ''
    apiSecret.value = ''
    toast.success(t('wallet.bybit.keySaved'))
    await status.refetch()
  } catch (err) {
    toast.error(t('wallet.bybit.keySaveFailed'), { description: parseError(err).message })
  } finally {
    busy.value = false
  }
}

async function remove() {
  busy.value = true
  try {
    await call('clear_exchange_key', { storeId: props.storeId })
    toast.success(t('wallet.bybit.keyRemoved'))
    await status.refetch()
  } catch (err) {
    toast.error(t('wallet.bybit.keyRemoveFailed'), { description: parseError(err).message })
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <div class="flex flex-col gap-4">
    <p class="text-sm text-muted-foreground">{{ t('wallet.bybit.hint') }}</p>

    <div v-if="hasKey" class="flex items-center gap-2">
      <Badge variant="secondary">{{ hint }}</Badge>
      <Button variant="ghost" :disabled="busy" @click="remove">
        {{ t('wallet.bybit.remove') }}
      </Button>
    </div>

    <!-- Key, secret and the button on one row: it is a two-field form. -->
    <form v-else class="grid items-end gap-3 sm:grid-cols-[1fr_1fr_auto]" @submit.prevent="save">
      <div class="grid gap-1.5">
        <Label for="bk">{{ t('wallet.bybit.apiKey') }}</Label>
        <Input id="bk" v-model="apiKey" autocomplete="off" spellcheck="false" />
      </div>
      <div class="grid gap-1.5">
        <Label for="bs">{{ t('wallet.bybit.apiSecret') }}</Label>
        <Input id="bs" v-model="apiSecret" type="password" autocomplete="new-password" spellcheck="false" />
      </div>
      <Button type="submit" :disabled="busy || !apiKey || !apiSecret">
        {{ t('wallet.bybit.save') }}
      </Button>
    </form>
  </div>
</template>
