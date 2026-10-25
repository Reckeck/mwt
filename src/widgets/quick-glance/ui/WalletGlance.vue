<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'

import { entityKey } from '@/shared/lib/entity-link'
import { openQuickAdd } from '@/shared/lib/quick-add'
import { Button } from '@/shared/ui/button'
import { Glance } from '@/shared/ui/glance'

import type { WalletGlanceProps } from '../quick-glance.types'
import WalletGlanceBody from './WalletGlanceBody.vue'

/**
 * A wallet's name in an operation row opened on the spot (Paths 1 · 4): its
 * value, the sub-accounts and the last three operations; then «Перевести»,
 * «Обменять» or «Открыть кошелёк →». The list under it stays where it was.
 */
const props = defineProps<WalletGlanceProps>()
const { t } = useI18n()

const open = ref(false)

function record(kind: TxKind) {
  open.value = false
  openQuickAdd({ storeId: props.storeId, kind })
}
</script>

<template>
  <Glance overlay="glance-wallet"
    v-model:open="open"
    :title="props.name"
    :open-to="{ name: 'wallet', params: { id: props.storeId } }"
    :open-label="t('glance.wallet.open')"
    :trigger-class="props.triggerClass"
    kind="wallet"
    :entity="entityKey({ kind: 'store', id: props.storeId })"
  >
    <slot>{{ props.name }}</slot>
    <template #body>
      <WalletGlanceBody :store-id="props.storeId" :name="props.name" />
    </template>
    <template #actions>
      <Button variant="outline" size="sm" data-glance-transfer @click="record('transfer')">{{ t('glance.wallet.transfer') }}</Button>
      <Button variant="outline" size="sm" class="max-md:hidden" data-glance-convert @click="record('convert')">{{ t('glance.wallet.convert') }}</Button>
    </template>
  </Glance>
</template>
