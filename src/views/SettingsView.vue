<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import Badge from '@/components/ui/badge.vue'
import Button from '@/components/ui/button.vue'
import Card from '@/components/ui/card.vue'
import Input from '@/components/ui/input.vue'
import Label from '@/components/ui/label.vue'
import { call, parseError } from '@/lib/ipc'
import { setLocalePreference } from '@/locale'
import { useSettingsStore } from '@/stores/settings'
import { useTradeLookupStore } from '@/stores/tradeLookup'

const { t } = useI18n()
const settingsStore = useSettingsStore()
const lookup = useTradeLookupStore()

const bybitKey = ref('')
const bybitSecret = ref('')
const bybitError = ref<string | null>(null)
const bybitBusy = ref(false)
const bybitSaved = ref(false)

async function saveBybitKey() {
  bybitError.value = null
  bybitSaved.value = false
  bybitBusy.value = true
  try {
    await lookup.saveKey({ api_key: bybitKey.value, api_secret: bybitSecret.value })
    bybitKey.value = ''
    bybitSecret.value = ''
    bybitSaved.value = true
  } catch (e) {
    bybitError.value = parseError(e).message
  } finally {
    bybitBusy.value = false
  }
}

async function removeBybitKey() {
  bybitError.value = null
  bybitBusy.value = true
  try {
    await lookup.clearKey()
    bybitSaved.value = false
  } catch (e) {
    bybitError.value = parseError(e).message
  } finally {
    bybitBusy.value = false
  }
}

onMounted(() => {
  if (settingsStore.settings) void lookup.loadKeyStatus()
})

const baseCurrency = ref<string>(settingsStore.settings?.base_currency ?? 'usd')
const saveMsg = ref<'saved' | null>(null)
const localeError = ref(false)

const searches = useQuery({
  queryKey: ['prices'],
  queryFn: () => call<AssetSearchHit[]>('search_assets', { query: 'bitcoin' }),
})

const currencies = [
  { code: 'usd', label: 'USD' },
  { code: 'eur', label: 'EUR' },
  { code: 'rub', label: 'RUB' },
  { code: 'cny', label: 'CNY' },
]

const localeOptions: { code: Locale; label: string }[] = [
  { code: 'auto', label: 'settings.locale.auto' },
  { code: 'ru', label: 'settings.locale.ru' },
  { code: 'en', label: 'settings.locale.en' },
]

const currentLocale = () => settingsStore.settings?.locale ?? 'auto'

/**
 * Language applies immediately — the UI must switch while the user is still
 * looking at the control, so it is persisted on change rather than on Save.
 */
async function onLocaleChange(e: Event) {
  const next = (e.target as HTMLSelectElement).value as Locale
  localeError.value = false
  try {
    await setLocalePreference(next)
  } catch {
    localeError.value = true
  }
}

async function saveSettings() {
  const current = settingsStore.settings
  if (!current) return
  const input: UpdateSettingsInput = {
    theme: current.theme,
    locale: current.locale,
    base_currency: baseCurrency.value,
    display_decimals: current.display_decimals,
  }
  await call('update_settings', { input })
  await settingsStore.load()
  saveMsg.value = 'saved'
  setTimeout(() => (saveMsg.value = null), 2000)
}

function setTheme(th: Theme) {
  if (settingsStore.settings) settingsStore.settings.theme = th
}
</script>

<template>
  <div class="mx-auto grid max-w-3xl gap-4">
    <Card>
      <template #header>
        <p class="font-medium">{{ t('settings.title') }}</p>
      </template>

      <div class="grid gap-4">
        <div class="grid gap-1.5">
          <Label>{{ t('settings.theme.title') }}</Label>
          <div v-if="settingsStore.settings" class="grid grid-cols-3 gap-2">
            <button
              v-for="th in (['light', 'dark', 'auto'] as const)"
              :key="th"
              type="button"
              class="rounded-md border px-3 py-2 text-sm transition-colors hover:bg-accent"
              :class="settingsStore.settings.theme === th && 'border-primary bg-accent'"
              @click="setTheme(th)"
            >
              {{ t(`settings.theme.${th}`) }}
            </button>
          </div>
        </div>

        <div class="grid gap-1.5">
          <Label for="bcur">{{ t('settings.baseCurrency') }}</Label>
          <select
            id="bcur"
            v-model="baseCurrency"
            class="h-9 rounded-md border border-input bg-transparent px-3 text-sm"
          >
            <option v-for="c in currencies" :key="c.code" :value="c.code">{{ c.label }}</option>
          </select>
        </div>

        <div class="grid gap-1.5">
          <Label for="loc">{{ t('settings.language') }}</Label>
          <select
            id="loc"
            :value="currentLocale()"
            class="h-9 rounded-md border border-input bg-transparent px-3 text-sm"
            @change="onLocaleChange"
          >
            <option v-for="l in localeOptions" :key="l.code" :value="l.code">{{ t(l.label) }}</option>
          </select>
          <p v-if="localeError" class="text-xs text-destructive">{{ t('settings.localeSaveFailed') }}</p>
        </div>

        <div class="flex items-center gap-3">
          <Button @click="saveSettings">{{ t('actions.save') }}</Button>
          <Badge v-if="saveMsg === 'saved'" variant="default">✓</Badge>
        </div>
      </div>
    </Card>

    <Card>
      <template #header>
        <p class="font-medium">{{ t('settings.bybit.title') }}</p>
      </template>
      <div class="grid gap-3">
        <p class="text-sm text-muted-foreground">{{ t('settings.bybit.hint') }}</p>

        <div v-if="lookup.hasKey" class="flex items-center gap-2">
          <Badge variant="secondary">{{ lookup.key.hint }}</Badge>
          <Button variant="ghost" :disabled="bybitBusy" @click="removeBybitKey">
            {{ t('settings.bybit.remove') }}
          </Button>
        </div>

        <template v-else>
          <div class="grid gap-1.5">
            <Label for="bk">{{ t('settings.bybit.apiKey') }}</Label>
            <Input id="bk" v-model="bybitKey" autocomplete="off" spellcheck="false" />
          </div>
          <div class="grid gap-1.5">
            <Label for="bs">{{ t('settings.bybit.apiSecret') }}</Label>
            <Input
              id="bs"
              v-model="bybitSecret"
              type="password"
              autocomplete="new-password"
              spellcheck="false"
            />
          </div>
          <div class="flex items-center gap-3">
            <Button :disabled="bybitBusy || !bybitKey || !bybitSecret" @click="saveBybitKey">
              {{ t('settings.bybit.save') }}
            </Button>
            <Badge v-if="bybitSaved" variant="default">✓</Badge>
          </div>
        </template>

        <p v-if="bybitError" class="text-xs text-destructive">{{ bybitError }}</p>
      </div>
    </Card>

    <Card>
      <template #header>
        <p class="font-medium">{{ t('settings.prices') }}</p>
      </template>
      <div class="flex flex-wrap gap-2">
        <Badge v-for="(hit, i) in searches.data.value ?? []" :key="hit.symbol + i" variant="secondary">
          {{ hit.symbol }} {{ hit.name }}
        </Badge>
        <p v-if="!searches.data.value?.length" class="text-sm text-muted-foreground">{{ t('settings.prices') }}</p>
      </div>
    </Card>
  </div>
</template>