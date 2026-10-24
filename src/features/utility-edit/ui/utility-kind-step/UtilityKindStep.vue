<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { StoreNodeSelect } from '@/entities/store'
import { Button } from '@/shared/ui/button'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { cn } from '@/shared/lib/utils'

import { useUtilityForm } from '../../model/use-utility-form'
import { KIND_ICONS, PICK_CARD_CLASS, UTILITY_KINDS, type UtilityKindName } from '../../utility-edit.constants'

/**
 * Step 1 (Wallets-3 · 14): the name and how the item is counted — a card per
 * kind with what it means; a second click on the picked one moves on.
 */
const props = defineProps<{ askProperty: boolean }>()
const emit = defineEmits<{ next: [] }>()
const { t } = useI18n()
const { name, kind, propertyId, properties, form } = useUtilityForm()

function choose(value: UtilityKindName) {
  if (kind.value === value) emit('next')
  else kind.value = value
}
</script>

<template>
  <FormField for="utility-name" :label="t('utilities.name')" required :error="form.error('name')">
    <Input id="utility-name" v-model="name" :placeholder="t('utilities.namePlaceholder')" />
  </FormField>
  <FormField v-if="props.askProperty" for="utility-property" :label="t('utilities.property')" required :error="form.error('property')">
    <StoreNodeSelect
      id="utility-property"
      v-model="propertyId"
      :nodes="properties"
      flat
      :placeholder="t('utilities.pickProperty')"
      :invalid="Boolean(form.error('property'))"
    />
    <p v-if="!properties.length" class="text-xs text-muted-foreground">{{ t('utilities.noProperty') }}</p>
  </FormField>
  <div class="grid gap-1.5">
    <Label>{{ t('utilities.kind') }}</Label>
    <div class="grid gap-2 @lg:grid-cols-3" data-utility-kinds>
      <Button
        v-for="entry in UTILITY_KINDS"
        :key="entry"
        type="button"
        variant="outline"
        :aria-pressed="kind === entry"
        :class="cn(PICK_CARD_CLASS, kind === entry && 'border-primary bg-primary/5 hover:bg-primary/5')"
        @click="choose(entry)"
      >
        <span class="flex items-center gap-2 text-sm font-medium">
          <component :is="KIND_ICONS[entry]" class="size-4 shrink-0" />{{ t(`utilities.kindName.${entry}`) }}
        </span>
        <span class="text-xs text-muted-foreground">{{ t(`utilities.kindHint.${entry}`) }}</span>
      </Button>
    </div>
  </div>
</template>
