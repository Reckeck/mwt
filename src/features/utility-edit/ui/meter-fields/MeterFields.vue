<script setup lang="ts">
import { computed, nextTick } from 'vue'
import { useI18n } from 'vue-i18n'
import { Check } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { AmountInput } from '@/shared/ui/amount-input'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'
import { currencySymbol, formatQuantity } from '@/shared/lib/format'
import { fillFormula, prettyFormula } from '@/shared/lib/utility-formula'

import { FORMULA_VARIABLES, type MeterMode } from '../../meter-formula'
import { useUtilityForm } from '../../model/use-utility-form'
import { ZONE_COUNTS, type ZoneCount } from '../../utility-edit.constants'

/**
 * Step 2 of a meter (Wallets-3 · 15–16): how many readings it has, the unit and
 * the tariff, and how the sum is made — consumption × tariff with an example,
 * or a formula with the names to insert and a check on the last readings.
 */
const { t, locale } = useI18n()
const { unit, tariff, meterMode, formula, zoneNames, zoneCount, setZoneCount, currency, preview, sample, form } = useUtilityForm()

/** Whole sums without cents, the rest with. */
const decimalsOf = (value: string) => (Number(value) % 1 ? 2 : 0)
const quantity = (value: string) => formatQuantity(value, 3)
const ru = computed(() => locale.value === 'ru')

/** «Пример: расход 100 м³ — 4 900 ₽. Показание одно: …» */
const example = computed(() => {
  const amount = preview.value?.amount
  if (!amount || !(Number(tariff.value) > 0)) return null
  const zones = zoneNames.value.length
  return {
    sumKey: zones ? 'utilityEdit.exampleZones' : 'utilityEdit.example',
    amount,
    how: zones ? t('utilityEdit.exampleZonesHow', { names: zoneNames.value.join(', ') }) : t('utilityEdit.exampleOneHow'),
  }
})
/** «Проверка на последних показаниях: (209 − 205) × 3,5 × 10 = 140,00 ₽». */
const check = computed(() => {
  const result = preview.value
  if (!result?.amount || result.issue || !formula.value.trim()) return null
  const reading = result.input.readings[0]
  const text =
    result.input.readings.length === 1 && reading
      ? fillFormula(prettyFormula(formula.value), {
          current: quantity(reading.current),
          previous: quantity(reading.previous),
          used: quantity(String(Math.max(0, Number(reading.current) - Number(reading.previous)))),
          tariff: quantity(tariff.value || '0'),
        })
      : prettyFormula(formula.value)
  return { label: t(sample.value ? 'utilityEdit.checkLast' : 'utilityEdit.checkSample'), text: `${text} =`, amount: result.amount }
})

/** Puts a name where the caret is in the formula field. */
async function insert(word: string) {
  const field = document.querySelector<HTMLInputElement>('#utility-formula')
  const at = field?.selectionStart ?? formula.value.length
  const before = formula.value.slice(0, at)
  const gap = before && !/[\s(]$/.test(before) ? ' ' : ''
  formula.value = `${before}${gap}${word}${formula.value.slice(at)}`
  await nextTick()
  const caret = at + gap.length + word.length
  field?.focus()
  field?.setSelectionRange(caret, caret)
}
const onZones = (value: unknown) => value && setZoneCount(value as ZoneCount)
function onMode(value: unknown) {
  if (value) meterMode.value = value as MeterMode
}
</script>

<template>
  <div class="grid gap-1.5" :data-invalid="form.invalidAttr('zones')">
    <Label>{{ t('utilityEdit.zones') }}</Label>
    <ToggleGroup :model-value="zoneCount ?? undefined" type="single" variant="segmented" :spacing="1" class="justify-self-start" @update:model-value="onZones">
      <ToggleGroupItem v-for="count in ZONE_COUNTS" :key="count" :value="count" class="px-3">{{ t(`utilityEdit.zoneCount.${count}`) }}</ToggleGroupItem>
    </ToggleGroup>
    <p class="text-xs text-muted-foreground">{{ t('utilityEdit.zonesHint') }}</p>
    <p v-if="form.error('zones')" class="text-xs text-destructive" role="alert">{{ form.error('zones') }}</p>
  </div>
  <div class="grid items-start gap-3 @lg:grid-cols-2">
    <FormField for="utility-unit" :label="t('utilities.unit')" required :error="form.error('unit')">
      <Input id="utility-unit" v-model="unit" />
    </FormField>
    <FormField for="utility-tariff" :label="t('utilityEdit.tariff')" :required="meterMode === 'tariff'" :error="form.error('tariff')">
      <AmountInput id="utility-tariff" v-model="tariff">
        <template #suffix><Badge variant="secondary" class="font-normal">{{ currencySymbol(currency) }}</Badge></template>
      </AmountInput>
      <p v-if="meterMode === 'formula'" class="text-xs text-muted-foreground">{{ t('utilityEdit.tariffInFormula') }}</p>
    </FormField>
  </div>
  <div class="grid gap-1.5">
    <Label>{{ t('utilityEdit.sumBy') }}</Label>
    <ToggleGroup data-utility-mode :model-value="meterMode" type="single" variant="segmented" :spacing="1" class="justify-self-start" @update:model-value="onMode">
      <ToggleGroupItem value="tariff" class="px-3">{{ t('utilities.mode.tariff') }}</ToggleGroupItem>
      <ToggleGroupItem value="formula" class="px-3">{{ t('utilities.mode.formula') }}</ToggleGroupItem>
    </ToggleGroup>
  </div>
  <template v-if="meterMode === 'formula'">
    <FormField for="utility-formula" :label="t('utilities.formula')" required :error="form.error('formula')">
      <Input id="utility-formula" v-model="formula" :placeholder="t('utilities.formulaPlaceholder')" autocomplete="off" spellcheck="false" />
    </FormField>
    <div class="flex flex-wrap items-center gap-1.5" data-formula-names>
      <span class="text-xs text-muted-foreground">{{ t('utilityEdit.insert') }}</span>
      <Button
        v-for="[word, latin] in FORMULA_VARIABLES"
        :key="word"
        type="button"
        variant="outline"
        size="sm"
        class="h-7 px-2 text-xs font-normal text-muted-foreground touch-hit"
        @click="insert(ru ? word : latin)"
      >
        {{ ru ? word : latin }}
      </Button>
    </div>
    <p class="text-xs text-muted-foreground">{{ t('utilityEdit.formulaHint') }}</p>
  </template>
  <div v-if="meterMode === 'tariff' ? example : check" class="flex items-start gap-2 rounded-lg bg-muted p-3 text-sm" data-utility-example>
    <Check class="mt-0.5 size-4 shrink-0" />
    <p v-if="meterMode === 'tariff' && example">
      <i18n-t :keypath="example.sumKey" tag="span">
        <template #unit>{{ unit }}</template>
        <template #amount><Money :amount="example.amount" :currency="currency" :decimals="decimalsOf(example.amount)" /></template>
      </i18n-t>
      {{ example.how }}
    </p>
    <p v-else-if="check" class="tabular-nums">
      {{ check.label }} {{ check.text }} <Money :amount="check.amount" :currency="currency" :decimals="2" />
    </p>
  </div>
</template>
