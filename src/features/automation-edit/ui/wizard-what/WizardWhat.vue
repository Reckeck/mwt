<script setup lang="ts">
import type { Component } from 'vue'
import { useI18n } from 'vue-i18n'
import { Bell, Repeat2, TrendingUp, Workflow } from 'lucide-vue-next'

import { useIsPhone } from '@/shared/lib/phone'

import {
  WIZARD_PHONE_STARTERS,
  WIZARD_STARTERS,
  WIZARD_TYPES,
  type WizardStarter,
  type WizardType,
} from '../../automation-edit.constants'
import { useAutomationWizard } from '../../model/use-automation-wizard'
import { WizardOption } from '../wizard-option'

/**
 * Step 1 (Auto-1 · 6): what should happen — four kinds, two in a row; a click
 * picks, a second one or «Далее» moves on. Below, ready-made starts. A phone
 * lists them, and a tap moves on at once.
 */
const { t } = useI18n()
const phone = useIsPhone()
const { type, goNext, startFrom } = useAutomationWizard()

const ICONS: Record<WizardType, Component> = { repeat: Repeat2, reminder: Bell, alert: TrendingUp, scheme: Workflow }

function pick(value: WizardType) {
  if (phone.value || type.value === value) {
    type.value = value
    goNext()
  } else type.value = value
}
const starters = (): readonly WizardStarter[] => (phone.value ? WIZARD_PHONE_STARTERS : WIZARD_STARTERS)
</script>

<template>
  <div class="grid gap-3.5 max-md:gap-0" data-wizard-what>
    <div class="grid gap-2.5 md:grid-cols-2 max-md:gap-0">
      <WizardOption
        v-for="entry in WIZARD_TYPES"
        :key="entry"
        :icon="ICONS[entry]"
        :title="t(`automationWizard.type.${entry}.title`)"
        :hint="t(`automationWizard.type.${entry}.hint`)"
        :selected="type === entry"
        :row="phone"
        :data-wizard-type="entry"
        @click="pick(entry)"
      />
    </div>
    <p class="mt-1 text-sm font-semibold max-md:mt-3.5">{{ t('automationWizard.starters') }}</p>
    <div class="grid gap-2.5 md:grid-cols-2 max-md:gap-0">
      <WizardOption
        v-for="entry in starters()"
        :key="entry"
        :title="t(`automationWizard.starter.${entry}.title`)"
        :hint="t(`automationWizard.starter.${entry}.hint`)"
        :row="phone"
        :data-wizard-starter="entry"
        @click="startFrom(entry)"
      />
    </div>
  </div>
</template>
