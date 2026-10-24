<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { FormDialog } from '@/shared/ui/form-dialog'
import { FormField } from '@/shared/ui/form-field'
import { Label } from '@/shared/ui/label'
import { Input } from '@/shared/ui/input'
import { Textarea } from '@/shared/ui/textarea'
import { DateTimePicker } from '@/shared/ui/date-time-picker'

import { useAdjustment } from './model/use-adjustment'
import type { AdjustmentDialogProps } from './store-adjust.types'
import { AdjustmentAssetAdder } from './ui/adjustment-asset-adder'
import { AdjustmentRowsTable } from './ui/adjustment-rows-table'
import { AdjustmentSimple } from './ui/adjustment-simple'

/** «Сверить с реальностью»: type what is really there, the difference is booked as a correction. */
const props = withDefaults(defineProps<AdjustmentDialogProps>(), { transaction: null })
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const { fiatOnly, simple, inApp, node, title, timestamp, comment, rows, draft, changed, form, save } = useAdjustment(props, open)
</script>

<template>
  <FormDialog
    v-model:open="open"
    :title="simple ? t('adjustment.open') : title"
    :description="simple ? undefined : t('adjustment.hint')"
    :size="simple ? 'form' : 'lg'"
    form-id="adjustment-form"
    :submit-label="simple ? t('adjustment.submit', 0) : t('adjustment.submit', { count: changed.length })"
    :pending="save.isPending.value"
    :phone-submit-only="simple"
    @submit="form.submit(save.mutate, $event)"
  >
    <AdjustmentSimple
      v-if="simple && rows[0]"
      :row="rows[0]"
      :in-app="inApp"
      :cash="node?.store.kind === 'cash'"
      :error="form.error('rows')"
    />
    <template v-else>
    <FormField
      for="adjustment-date"
      :label="t('adjustment.date')"
      class="sm:max-w-64"
      required
      :error="form.error('timestamp')"
      :hint="t('adjustment.dateHint')"
    >
      <DateTimePicker id="adjustment-date" v-model="timestamp" mode="datetime" class="px-2" />
    </FormField>

    <AdjustmentRowsTable v-model:rows="rows" :invalid="Boolean(form.error('rows'))" :loaded="draft.isSuccess.value" />
    <p v-if="form.error('rows')" class="-mt-2 text-xs text-destructive" role="alert">{{ form.error('rows') }}</p>

    <AdjustmentAssetAdder
      :fiat-only="fiatOnly"
      :taken="rows.map((row) => row.asset)"
      @add="(asset) => rows.push({ asset, before: '0', actual: '' })"
    />
    </template>

    <div class="grid gap-1.5">
      <Label for="adjustment-comment">{{ t('form.comment') }}</Label>
      <Input v-if="simple" id="adjustment-comment" v-model="comment" :placeholder="t('adjustment.simple.commentHint')" />
      <Textarea v-else id="adjustment-comment" v-model="comment" rows="1" :placeholder="t('adjustment.commentHint')" />
    </div>

    <p v-if="!simple && !changed.length && !form.error('rows')" class="text-xs text-muted-foreground">{{ t('adjustment.nothing') }}</p>
  </FormDialog>
</template>
