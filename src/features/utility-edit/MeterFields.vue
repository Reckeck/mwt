<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { AmountInput } from '@/shared/ui/amount-input'
import { Button } from '@/shared/ui/button'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Textarea } from '@/shared/ui/textarea'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'

import { FORMULA_VARIABLES, type MeterMode, zoneKey, zoneList } from './meter-formula'
import type { MeterFieldsProps } from './utility-edit.types'

/**
 * A meter's pricing: consumption × tariff, or the user's own formula over the
 * readings («(показание - предыдущее) * 3.5 * 10»), with tariff zones for a
 * day/night meter. The preview (from `useFormulaPreview`) prices 100 units in every zone.
 */
const props = defineProps<MeterFieldsProps>()
const unit = defineModel<string>('unit', { required: true })
const tariff = defineModel<string>('tariff', { required: true })
const mode = defineModel<MeterMode>('mode', { required: true })
const formula = defineModel<string>('formula', { required: true })
const zones = defineModel<string>('zones', { required: true })

const { t, locale } = useI18n()

const zoneNames = computed(() => zoneList(zones.value))
const names = computed(() => {
  const ru = locale.value === 'ru'
  const own = FORMULA_VARIABLES.map(([nameRu, nameEn]) => (ru ? nameRu : nameEn))
  const used = ru ? 'расход' : 'used'
  return [...own, ...zoneNames.value.map((zone) => `${used}_${zoneKey(zone)}`)]
})

function insert(name: string) {
  const text = formula.value.trimEnd()
  formula.value = text ? `${text} ${name}` : name
}
</script>

<template>
  <div class="grid gap-3">
    <ToggleGroup v-model="mode" type="single" variant="outline" size="sm" :aria-label="t('utilities.meterMode')" class="justify-start">
      <ToggleGroupItem value="tariff">{{ t('utilities.mode.tariff') }}</ToggleGroupItem>
      <ToggleGroupItem value="formula">{{ t('utilities.mode.formula') }}</ToggleGroupItem>
    </ToggleGroup>
    <div class="grid gap-3 @sm:grid-cols-3">
      <FormField for="utility-unit" :label="t('utilities.unit')" required :error="props.unitError">
        <Input id="utility-unit" v-model="unit" />
      </FormField>
      <FormField for="utility-tariff" :label="t('utilities.tariff', { unit })" :required="mode === 'tariff'" :error="props.tariffError">
        <AmountInput id="utility-tariff" v-model="tariff" :placeholder="mode === 'formula' ? t('goal.optional') : undefined" />
      </FormField>
      <FormField for="utility-zones" :label="t('utilities.zones')" :error="props.zonesError">
        <Input id="utility-zones" v-model="zones" :placeholder="t('utilities.zonesPlaceholder')" />
      </FormField>
    </div>
    <p class="text-xs text-muted-foreground">{{ t('utilities.zonesHint') }}</p>

    <template v-if="mode === 'formula'">
      <FormField for="utility-formula" :label="t('utilities.formula')" required :error="props.formulaError">
        <Textarea id="utility-formula" v-model="formula" rows="2" class="font-mono" :placeholder="t('utilities.formulaPlaceholder')" />
      </FormField>
      <div class="flex flex-wrap gap-1" :aria-label="t('utilities.formulaNames')">
        <Button v-for="name in names" :key="name" type="button" variant="outline" size="sm" class="h-6 px-2 font-mono" @click="insert(name)">
          {{ name }}
        </Button>
      </div>
      <p class="text-xs text-muted-foreground [overflow-wrap:anywhere]">{{ t('utilities.formulaHint') }}</p>
    </template>
    <p v-if="props.preview?.amount" class="text-xs text-muted-foreground">
      {{ t('utilities.formulaPreview', { unit }) }}
      <Money :amount="props.preview.amount" :currency="props.currency" :convert="false" class="font-medium text-foreground" />
    </p>
  </div>
</template>
