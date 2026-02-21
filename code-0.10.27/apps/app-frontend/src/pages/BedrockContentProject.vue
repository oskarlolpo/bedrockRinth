<script setup lang="ts">
import { DownloadIcon, ExternalIcon } from '@modrinth/assets'
import {
	ButtonStyled,
	defineMessages,
	injectNotificationManager,
	LoadingIndicator,
	useVIntl,
} from '@modrinth/ui'
import { computed, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import InstanceIndicator from '@/components/ui/InstanceIndicator.vue'
import {
	get_bedrock_content_details,
	install_bedrock_content,
} from '@/helpers/metadata.js'
import { get as getInstance } from '@/helpers/profile.js'

const route = useRoute()
const router = useRouter()
const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()

const messages = defineMessages({
	backToInstance: { id: 'project.back-to-instance', defaultMessage: 'Back to instance' },
	description: { id: 'project.description', defaultMessage: 'Description' },
	gallery: { id: 'project.gallery', defaultMessage: 'Gallery' },
	install: { id: 'browse.install', defaultMessage: 'Install' },
	installing: { id: 'browse.installing', defaultMessage: 'Installing' },
	openSourcePage: { id: 'browse.open-source-page', defaultMessage: 'Open source page' },
})

const instancePath = computed(() => {
	const raw = route.query.i
	if (Array.isArray(raw)) return raw[0] ?? ''
	return raw ? String(raw) : ''
})
const pageUrl = computed(() => {
	const raw = route.query.p
	if (Array.isArray(raw)) return raw[0] ?? ''
	return raw ? String(raw) : ''
})
const contentType = computed(() => String(route.params.contentType || 'textures').toLowerCase())

const instance = ref<any | null>(null)
const loading = ref(false)
const installing = ref(false)
const activeTab = ref<'description' | 'gallery'>('description')
const details = ref<any | null>(null)

async function init() {
	if (!instancePath.value || !pageUrl.value) {
		await router.replace('/library')
		return
	}
	loading.value = true
	try {
		instance.value = await getInstance(instancePath.value).catch(() => null)
		details.value = await get_bedrock_content_details(contentType.value, pageUrl.value)
	} catch (error) {
		handleError(error)
	} finally {
		loading.value = false
	}
}

async function install() {
	if (!instancePath.value || !pageUrl.value) return
	installing.value = true
	try {
		await install_bedrock_content(instancePath.value, contentType.value, pageUrl.value)
	} catch (error) {
		handleError(error)
	} finally {
		installing.value = false
	}
}

await init()
</script>

<template>
	<div class="flex flex-col gap-3 p-6">
		<template v-if="instance">
			<InstanceIndicator :instance="instance" />
		</template>

		<div v-if="loading" class="offline">
			<LoadingIndicator />
		</div>

		<template v-else-if="details">
			<div class="flex items-start justify-between gap-4">
				<div class="flex gap-4 min-w-0">
					<img
						v-if="details.image_url"
						:src="details.image_url"
						alt="preview"
						class="w-24 h-24 rounded-xl object-cover shrink-0"
					/>
					<div class="min-w-0">
						<h1 class="m-0 text-3xl font-extrabold">{{ details.title }}</h1>
						<div class="text-secondary mt-2">{{ details.supported_versions?.join(', ') }}</div>
					</div>
				</div>
				<div class="flex items-center gap-2 shrink-0">
					<ButtonStyled color="brand" type="outlined">
						<button :disabled="installing" @click="install">
							<DownloadIcon />
							{{ installing ? formatMessage(messages.installing) : formatMessage(messages.install) }}
						</button>
					</ButtonStyled>
					<ButtonStyled type="transparent">
						<a :href="details.page_url" target="_blank" rel="noreferrer">
							{{ formatMessage(messages.openSourcePage) }} <ExternalIcon />
						</a>
					</ButtonStyled>
				</div>
			</div>

			<div class="flex gap-2">
				<button
					class="px-4 py-2 rounded-full border-none cursor-pointer"
					:class="activeTab === 'description' ? 'bg-brand-highlight text-brand' : 'bg-bg-raised text-secondary'"
					@click="activeTab = 'description'"
				>
					{{ formatMessage(messages.description) }}
				</button>
				<button
					class="px-4 py-2 rounded-full border-none cursor-pointer"
					:class="activeTab === 'gallery' ? 'bg-brand-highlight text-brand' : 'bg-bg-raised text-secondary'"
					@click="activeTab = 'gallery'"
				>
					{{ formatMessage(messages.gallery) }}
				</button>
				<div class="ml-auto">
					<ButtonStyled type="transparent">
						<button @click="router.push({ path: `/instance/${instancePath}` })">
							{{ formatMessage(messages.backToInstance) }}
						</button>
					</ButtonStyled>
				</div>
			</div>

			<section v-if="activeTab === 'description'" class="p-6 rounded-2xl bg-bg-raised card-shadow">
				<div class="whitespace-pre-wrap leading-relaxed">
					{{ details.description }}
				</div>
			</section>

			<section v-else class="grid grid-cols-2 gap-4">
				<img
					v-for="img in details.gallery_images || []"
					:key="img"
					:src="img"
					alt="gallery"
					class="rounded-xl w-full object-cover"
				/>
			</section>
		</template>
	</div>
</template>
