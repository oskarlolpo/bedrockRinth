<script setup>
import { PlusIcon } from '@modrinth/assets'
import { Button, defineMessages, injectNotificationManager, useVIntl } from '@modrinth/ui'
import { onUnmounted, ref, shallowRef } from 'vue'
import { useRoute } from 'vue-router'

import { NewInstanceImage } from '@/assets/icons'
import InstanceCreationModal from '@/components/ui/InstanceCreationModal.vue'
import NavTabs from '@/components/ui/NavTabs.vue'
import { profile_listener } from '@/helpers/events.js'
import { list } from '@/helpers/profile.js'
import { useBreadcrumbs } from '@/store/breadcrumbs.js'

const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const route = useRoute()
const breadcrumbs = useBreadcrumbs()

const messages = defineMessages({
	library: {
		id: 'app.nav.library',
		defaultMessage: 'Library',
	},
	allInstances: {
		id: 'library.tabs.all',
		defaultMessage: 'All instances',
	},
	downloaded: {
		id: 'library.tabs.downloaded',
		defaultMessage: 'Downloaded',
	},
	custom: {
		id: 'library.tabs.custom',
		defaultMessage: 'Custom',
	},
	sharedWithMe: {
		id: 'library.tabs.shared',
		defaultMessage: 'Shared with me',
	},
	saved: {
		id: 'library.tabs.saved',
		defaultMessage: 'Saved',
	},
	noInstances: {
		id: 'library.empty.no-instances',
		defaultMessage: 'No instances found',
	},
	createInstance: {
		id: 'app.nav.create-instance',
		defaultMessage: 'Create new instance',
	},
})

breadcrumbs.setRootContext({ name: formatMessage(messages.library), link: route.path })

const instances = shallowRef(await list().catch(handleError))

const offline = ref(!navigator.onLine)
window.addEventListener('offline', () => {
	offline.value = true
})
window.addEventListener('online', () => {
	offline.value = false
})

const unlistenProfile = await profile_listener(async () => {
	instances.value = await list().catch(handleError)
})
onUnmounted(() => {
	unlistenProfile()
})
</script>

<template>
	<div class="p-6 flex flex-col gap-3">
		<h1 class="m-0 text-2xl hidden">{{ formatMessage(messages.library) }}</h1>
		<NavTabs
			:links="[
				{ label: formatMessage(messages.allInstances), href: `/library` },
				{ label: formatMessage(messages.downloaded), href: `/library/downloaded` },
				{ label: formatMessage(messages.custom), href: `/library/custom` },
				{ label: formatMessage(messages.sharedWithMe), href: `/library/shared`, shown: false },
				{ label: formatMessage(messages.saved), href: `/library/saved`, shown: false },
			]"
		/>
		<template v-if="instances.length > 0">
			<RouterView :instances="instances" />
		</template>
		<div v-else class="no-instance">
			<div class="icon">
				<NewInstanceImage />
			</div>
			<h3>{{ formatMessage(messages.noInstances) }}</h3>
			<Button color="primary" :disabled="offline" @click="$refs.installationModal.show()">
				<PlusIcon />
				{{ formatMessage(messages.createInstance) }}
			</Button>
			<InstanceCreationModal ref="installationModal" />
		</div>
	</div>
</template>

<style lang="scss" scoped>
.no-instance {
	display: flex;
	flex-direction: column;
	align-items: center;
	justify-content: center;
	height: 100%;
	gap: var(--gap-md);

	p,
	h3 {
		margin: 0;
	}

	.icon {
		svg {
			width: 10rem;
			height: 10rem;
		}
	}
}
</style>
