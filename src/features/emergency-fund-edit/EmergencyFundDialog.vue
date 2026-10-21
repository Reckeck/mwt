<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { useStoreTree } from '@/shared/api/queries'
import { StoreNodeSelect } from '@/entities/store'
import { FormDialog } from '@/shared/ui/form-dialog'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'

import { FUND_MONTHS_MAX, useFundForm } from './model/use-fund-form'

/** The fund's accounts and months, from the checkup (the wallet switch is the other way in). */
const props = defineProps<{ fund: EmergencyFund }>()
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const tree = useStoreTree(open)
const { storeIds, targetMonths, avgMonths, form, set, save } = useFundForm(() => props.fund, open)
</script>

<template>
  <FormDialog
    v-model:open="open"
    :title="t('emergency.dialog.title')"
    :description="t('emergency.dialog.description')"
    form-id="emergency-fund-form"
    :pending="set.isPending.value"
    @submit="form.submit(save, $event)"
  >
    <FormField for="emergency-accounts" :label="t('emergency.dialog.accounts')" :hint="t('emergency.dialog.accountsHint')">
      <StoreNodeSelect
        id="emergency-accounts"
        v-model:ids="storeIds"
        :nodes="tree.data.value ?? []"
        purpose="savings"
        multiple
        :placeholder="t('emergency.dialog.none')"
      />
    </FormField>
    <div class="grid gap-4 sm:grid-cols-2">
      <FormField
        for="emergency-target"
        :label="t('emergency.dialog.target')"
        :hint="t('emergency.dialog.targetHint')"
        :error="form.error('target')"
        required
      >
        <Input id="emergency-target" v-model="targetMonths" type="text" inputmode="numeric" min="1" :max="FUND_MONTHS_MAX" />
      </FormField>
      <FormField
        for="emergency-avg"
        :label="t('emergency.dialog.avg')"
        :hint="t('emergency.dialog.avgHint')"
        :error="form.error('avg')"
        required
      >
        <Input id="emergency-avg" v-model="avgMonths" type="text" inputmode="numeric" min="1" :max="FUND_MONTHS_MAX" />
      </FormField>
    </div>
  </FormDialog>
</template>
