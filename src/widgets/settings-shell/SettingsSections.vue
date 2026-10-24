<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { AppSettingsPanel } from '@/widgets/app-settings'
import { BudgetSettings } from '@/widgets/budget-settings'
import { CategorizationRules } from '@/widgets/categorization-rules'
import { CategorySummary } from '@/widgets/category-summary'
import { CategoryManager } from '@/widgets/category-manager'
import { CurrencySettings } from '@/widgets/currency-settings'
import { DeviceSettings } from '@/widgets/device-settings'
import { EnrichmentSettings } from '@/widgets/enrichment-settings'
import { FxRates } from '@/widgets/fx-rates'
import { InflationSettings } from '@/widgets/inflation-settings'
import { NotificationSettings } from '@/widgets/notification-settings'
import { QuickAddSettings } from '@/widgets/quick-add-settings'
import { SyncDevices, SyncHistory, SyncSettings, SyncTransfer } from '@/widgets/sync-settings'
import { VaultDevTools } from '@/widgets/vault-dev-tools'
import { VaultDirection } from '@/widgets/vault-direction'
import { VaultJsonEditor } from '@/widgets/vault-json-editor'
import { VaultSettings } from '@/widgets/vault-settings'
import { SettingsSection } from '@/shared/ui/settings-section'

import type { SettingsSection as SectionDef } from './settings-shell.constants'
import HelpWhatsNew from './HelpWhatsNew.vue'
import HotkeySettings from './HotkeySettings.vue'
import TourSettings from './TourSettings.vue'

/** One widget per section id; the shell decides which sections a tab shows. */
const props = defineProps<{ sections: SectionDef[] }>()
const { t } = useI18n()
</script>

<template>
  <div class="grid grid-cols-1 gap-4">
    <section
      v-for="section in props.sections"
      :key="section.id"
      :data-section="section.id"
      class="grid min-w-0 scroll-mt-4 grid-cols-1"
    >
      <SettingsSection v-if="section.id === 'app'" :title="t('settingsShell.section.view')" :description="t('settings.viewHint')"><AppSettingsPanel /></SettingsSection>
      <DeviceSettings v-else-if="section.id === 'device-app'" />
      <QuickAddSettings v-else-if="section.id === 'quick-add'" />
      <HotkeySettings v-else-if="section.id === 'hotkeys'" />
      <TourSettings v-else-if="section.id === 'tours'" />
      <HelpWhatsNew v-else-if="section.id === 'whats-new'" />
      <SettingsSection v-else-if="section.id === 'notifications'" :title="t('settingsShell.section.notifications')"><NotificationSettings /></SettingsSection>
      <VaultSettings v-else-if="section.id === 'vault'" />
      <VaultDirection v-else-if="section.id === 'direction'" />
      <CurrencySettings v-else-if="section.id === 'currencies'" />
      <CategorySummary v-else-if="section.id === 'categories-summary'" />
      <FxRates v-else-if="section.id === 'rates'" />
      <BudgetSettings v-else-if="section.id === 'budget'" />
      <CategoryManager v-else-if="section.id === 'categories'" />
      <CategorizationRules v-else-if="section.id === 'rules'" />
      <InflationSettings v-else-if="section.id === 'inflation'" />
      <EnrichmentSettings v-else-if="section.id === 'enrichment'" />
      <SyncSettings v-else-if="section.id === 'sync'" />
      <SyncDevices v-else-if="section.id === 'sync-devices'" />
      <SyncHistory v-else-if="section.id === 'sync-history'" />
      <SyncTransfer v-else-if="section.id === 'sync-transfer'" />
      <VaultDevTools v-else-if="section.id === 'devtools'">
        <template #viewer>
          <VaultJsonEditor />
        </template>
      </VaultDevTools>
    </section>
  </div>
</template>
