<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Plus } from 'lucide-vue-next'

import { AssetInput } from '@/entities/asset'
import { CurrencyPicker } from '@/entities/currency'
import { Button } from '@/shared/ui/button'
import { FormField } from '@/shared/ui/form-field'
import { assetKey, fiatRef } from '@/shared/lib/asset-ref'
import { required, useFormValidation } from '@/shared/lib/form'

/** An asset the ledger does not know about yet: a currency on cash and bank, any asset elsewhere. */
const props = defineProps<{ fiatOnly: boolean; taken: AssetRef[] }>()
const emit = defineEmits<{ add: [asset: AssetRef] }>()

const { t } = useI18n()
const newCurrency = ref('')
const newAsset = ref<AssetRef | null>(null)
const picked = computed(() => (props.fiatOnly ? (newCurrency.value ? fiatRef(newCurrency.value) : null) : newAsset.value))

const adder = useFormValidation(() => ({
  asset:
    required(picked.value, 'pick') ??
    (props.taken.some((asset) => picked.value && assetKey(asset) === assetKey(picked.value)) ? t('validation.duplicateAsset') : null),
}))

function add() {
  if (!picked.value) return
  emit('add', picked.value)
  newCurrency.value = ''
  newAsset.value = null
  adder.reset()
}
</script>

<template>
  <FormField :label="t('adjustment.addAsset')" :error="adder.error('asset')">
    <div class="flex items-start gap-2">
      <div class="min-w-0 flex-1">
        <CurrencyPicker v-if="props.fiatOnly" v-model="newCurrency" :kinds="['fiat']" label="" />
        <AssetInput v-else v-model="newAsset" label="" />
      </div>
      <Button type="button" variant="outline" @click="adder.submit(add, $event)">
        <Plus class="size-4" />
        {{ t('actions.add') }}
      </Button>
    </div>
  </FormField>
</template>
