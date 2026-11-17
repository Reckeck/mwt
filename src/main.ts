import { createPinia } from 'pinia'
import { createApp } from 'vue'

import App from '@/App.vue'
import { i18n } from '@/shared/config/i18n'
import { VueQueryPlugin, queryClient } from '@/app/providers/query'
import { router } from '@/app/providers/router'
import { registerVaultClosedSubscribers } from '@/app/vault-closed-subscribers'
import { purgeLegacyRecents } from '@/shared/lib/purge-legacy-recents'
import '@/app/styles.css'

purgeLegacyRecents()

const app = createApp(App)

app.use(router)
app.use(createPinia())
app.use(VueQueryPlugin, { queryClient })
app.use(i18n)
registerVaultClosedSubscribers()

app.mount('#app')
