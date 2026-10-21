<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Plus, RotateCcw, Trash2 } from 'lucide-vue-next'

import { assetClassName } from '@/entities/asset-class'
import { AmountInput } from '@/shared/ui/amount-input'
import { Button } from '@/shared/ui/button'
import { Checkbox } from '@/shared/ui/checkbox'
import { FormDialog } from '@/shared/ui/form-dialog'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'

import { useClassesForm } from '../model/use-classes-form'

/** The baskets and their target shares; the shares must add up to 100. */
const props = defineProps<{ view: AssetClassesView | undefined }>()
const open = defineModel<boolean>('open', { required: true })
const { t } = useI18n()

const { rows, total, form, set, add, remove, resetPreset, save } = useClassesForm(() => props.view, open)
</script>

<template>
  <FormDialog
    v-model:open="open"
    :title="t('assetClass.dialog.title')"
    :description="t('assetClass.dialog.description')"
    form-id="asset-class-form"
    :pending="set.isPending.value"
    @submit="form.submit(save, $event)"
  >
    <div class="grid gap-3">
      <div v-for="row in rows" :key="row.key" class="grid gap-1">
        <div class="flex flex-wrap items-center gap-2">
          <div class="min-w-40 flex-1" :data-invalid="form.invalidAttr(`name-${row.key}`)">
            <Input
              v-model="row.name"
              :aria-label="t('assetClass.dialog.name')"
              :placeholder="assetClassName({ id: row.id, name: '' }, t)"
            />
          </div>
          <div class="w-28" :data-invalid="form.invalidAttr(`pct-${row.key}`)">
            <AmountInput v-model="row.target" :decimals="2" :aria-label="t('assetClass.dialog.target')">
              <template #suffix><span class="px-2 text-xs text-muted-foreground">%</span></template>
            </AmountInput>
          </div>
          <div class="flex items-center gap-2 text-sm" :title="t('assetClass.dialog.capHint')">
            <Checkbox :id="`asset-class-cap-${row.key}`" v-model="row.cap" />
            <Label :for="`asset-class-cap-${row.key}`">{{ t('assetClass.dialog.cap') }}</Label>
          </div>
          <Button
            type="button"
            variant="ghost"
            size="icon"
            class="text-muted-foreground"
            :disabled="rows.length <= 1"
            :aria-label="t('assetClass.dialog.remove')"
            @click="remove(row.key)"
          >
            <Trash2 class="size-4" />
          </Button>
        </div>
        <p v-if="form.error(`name-${row.key}`) || form.error(`pct-${row.key}`)" class="text-xs text-destructive" role="alert">
          {{ form.error(`name-${row.key}`) ?? form.error(`pct-${row.key}`) }}
        </p>
      </div>
      <div class="flex flex-wrap items-center gap-2">
        <Button type="button" variant="outline" size="sm" @click="add">
          <Plus class="size-4" />
          {{ t('assetClass.dialog.add') }}
        </Button>
        <Button type="button" variant="ghost" size="sm" @click="resetPreset">
          <RotateCcw class="size-4" />
          {{ t('assetClass.dialog.preset') }}
        </Button>
      </div>
      <div :data-invalid="form.invalidAttr('total')" class="grid gap-1">
        <Label>{{ t('assetClass.dialog.total', { total }) }}</Label>
        <p v-if="form.error('total')" class="text-xs text-destructive" role="alert">{{ form.error('total') }}</p>
        <p v-else class="text-xs text-muted-foreground">{{ t('assetClass.dialog.capHint') }}</p>
      </div>
    </div>
  </FormDialog>
</template>
