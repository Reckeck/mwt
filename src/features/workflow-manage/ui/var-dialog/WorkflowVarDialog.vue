<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Select } from '@/shared/ui/select'

import { useWorkflowActions } from '../../model/use-workflow-actions'
import type { VarDialogProps } from './var-dialog.types'

/**
 * A shared variable by hand: a number, an amount or text that every workflow
 * reads with «Получить переменную» — change the share of savings in one place.
 */
const props = defineProps<VarDialogProps>()
const open = defineModel<boolean>('open', { default: false })

const { t } = useI18n()
const { setVar } = useWorkflowActions()

type Kind = 'number' | 'money' | 'text' | 'bool'
const name = ref('')
const kind = ref<Kind>('number')
const value = ref('')
const currency = ref('rub')

watch(open, (isOpen) => {
  if (!isOpen) return
  const current = props.variable
  name.value = current?.name ?? ''
  const parsed = current ? (JSON.parse(current.value) as WfValue) : null
  if (parsed?.type === 'money') {
    kind.value = 'money'
    value.value = parsed.value.amount
    currency.value = parsed.value.currency
  } else if (parsed?.type === 'text' || parsed?.type === 'bool' || parsed?.type === 'number') {
    kind.value = parsed.type
    value.value = String(parsed.value)
  } else {
    kind.value = 'number'
    value.value = ''
  }
})

const missing = computed(() => {
  if (!name.value.trim()) return t('workflow.vars.missingName')
  if ((kind.value === 'number' || kind.value === 'money') && !/^-?\d+([.,]\d+)?$/.test(value.value.trim())) {
    return t('workflow.vars.missingNumber')
  }
  return ''
})

function encoded(): string {
  const number = value.value.trim().replace(',', '.')
  const typed: WfValue =
    kind.value === 'money'
      ? { type: 'money', value: { amount: number, currency: currency.value.trim().toLowerCase() } }
      : kind.value === 'number'
        ? { type: 'number', value: number }
        : kind.value === 'bool'
          ? { type: 'bool', value: value.value === 'true' }
          : { type: 'text', value: value.value }
  return JSON.stringify(typed)
}

function submit() {
  if (missing.value) return
  setVar.mutate(
    { scope: props.variable?.scope ?? 'global', name: name.value.trim(), value: encoded() },
    { onSuccess: () => (open.value = false) },
  )
}
</script>

<template>
  <Dialog v-model:open="open" :title="t(props.variable ? 'workflow.vars.edit' : 'workflow.vars.new')" :description="t('workflow.vars.description')" size="sm">
    <form id="workflow-var" class="grid gap-4" @submit.prevent="submit">
      <div class="grid gap-1.5">
        <Label for="workflow-var-name">{{ t('workflow.vars.name') }}</Label>
        <Input id="workflow-var-name" v-model="name" :disabled="Boolean(props.variable)" />
      </div>
      <div class="grid gap-1.5">
        <Label for="workflow-var-kind">{{ t('workflow.vars.kind') }}</Label>
        <Select id="workflow-var-kind" v-model="kind">
          <option v-for="option in ['number', 'money', 'text', 'bool']" :key="option" :value="option">
            {{ t(`workflow.vars.kinds.${option}`) }}
          </option>
        </Select>
      </div>
      <div class="grid gap-1.5">
        <Label for="workflow-var-value">{{ t('workflow.vars.value') }}</Label>
        <Select v-if="kind === 'bool'" id="workflow-var-value" v-model="value">
          <option value="true">{{ t('workflow.vars.yes') }}</option>
          <option value="false">{{ t('workflow.vars.no') }}</option>
        </Select>
        <div v-else class="flex gap-2">
          <Input id="workflow-var-value" v-model="value" :inputmode="kind === 'text' ? 'text' : 'decimal'" />
          <Input v-if="kind === 'money'" v-model="currency" class="w-24" :aria-label="t('workflow.vars.currency')" />
        </div>
      </div>
      <p v-if="missing" class="text-xs text-muted-foreground">{{ missing }}</p>
    </form>
    <template #footer>
      <Button type="submit" form="workflow-var" :disabled="Boolean(missing) || setVar.isPending.value">{{ t('actions.save') }}</Button>
    </template>
  </Dialog>
</template>
