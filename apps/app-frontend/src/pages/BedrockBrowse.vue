<script setup lang="ts">
import { DownloadIcon, ExternalIcon, SearchIcon, TagsIcon, XIcon } from '@modrinth/assets'
import {
	Button,
	ButtonStyled,
	Checkbox,
	defineMessages,
	DropdownSelect,
	injectNotificationManager,
	LoadingIndicator,
	Pagination,
	useVIntl,
} from '@modrinth/ui'
import type { Ref } from 'vue'
import { computed, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import InstanceIndicator from '@/components/ui/InstanceIndicator.vue'
import NavTabs from '@/components/ui/NavTabs.vue'
import {
	get_bedrock_content,
	get_installed_bedrock_content,
	install_bedrock_content,
} from '@/helpers/metadata.js'
import { get as getInstance } from '@/helpers/profile.js'
import { useBreadcrumbs } from '@/store/breadcrumbs'

const route = useRoute()
const router = useRouter()
const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()

const messages = defineMessages({
	discoverContent: {
		id: 'browse.discover-content',
		defaultMessage: 'Discover content',
	},
	installToInstance: {
		id: 'browse.install-to-instance',
		defaultMessage: 'Install content to instance',
	},
	searchPlaceholder: {
		id: 'browse.search-placeholder',
		defaultMessage: 'Search {projectType}s...',
	},
	sortBy: {
		id: 'browse.sort-by',
		defaultMessage: 'Sort by',
	},
	view: {
		id: 'browse.view',
		defaultMessage: 'View',
	},
	noResults: {
		id: 'browse.no-results',
		defaultMessage: 'No content found for the current filters.',
	},
	openSourcePage: {
		id: 'browse.open-source-page',
		defaultMessage: 'Open source page',
	},
	install: {
		id: 'browse.install',
		defaultMessage: 'Install',
	},
	installing: {
		id: 'browse.installing',
		defaultMessage: 'Installing',
	},
	installed: {
		id: 'instance.project-status.installed',
		defaultMessage: 'Installed',
	},
	gameVersion: {
		id: 'search.filter.game_version',
		defaultMessage: 'Game version',
	},
	anyVersion: {
		id: 'common.any',
		defaultMessage: 'Any',
	},
	supportedVersions: {
		id: 'browse.supported-versions',
		defaultMessage: 'Supported versions',
	},
	textures: {
		id: 'browse.tabs.resource-packs',
		defaultMessage: 'Resource Packs',
	},
	shaders: {
		id: 'browse.tabs.shaders',
		defaultMessage: 'Shaders',
	},
	maps: {
		id: 'browse.tabs.maps',
		defaultMessage: 'Maps',
	},
	kindTextures: {
		id: 'browse.kind.textures',
		defaultMessage: 'texture packs',
	},
	kindShaders: {
		id: 'browse.kind.shaders',
		defaultMessage: 'shaders',
	},
	kindMaps: {
		id: 'browse.kind.maps',
		defaultMessage: 'maps',
	},
})

type BedrockItem = {
	title: string
	page_url: string
	image_url?: string | null
	description?: string
	kind: 'textures' | 'shaders' | 'maps'
	supported_versions: string[]
}

type BedrockPage = {
	page: number
	total_pages: number
	has_next: boolean
	items: BedrockItem[]
}

const contentType = computed(() =>
	(route.params.contentType as string)?.toLowerCase(),
)
const instancePath = computed(() => {
	const raw = route.query.i
	if (Array.isArray(raw)) return raw[0] ?? ''
	return raw ? String(raw) : ''
})

const query = ref('')
const loading = ref(false)
const installing = ref('')
const sitePage = ref(1)
const pageData = ref<BedrockPage>({ page: 1, total_pages: 1, has_next: false, items: [] })
const instance: Ref<any | null> = ref(null)
const selectedVersions = ref<string[]>([])
const selectedVersion = ref('')
const currentSortType = ref({ display: 'Relevance', name: 'relevance' })
const maxResults = ref(20)
const localPage = ref(1)
const installedUrls = ref<Set<string>>(new Set())
const installedTitleFingerprints = ref<Set<string>>(new Set())
const installProgress = ref<Record<string, number>>({})

const supportedKinds = ['textures', 'shaders', 'maps']

const tabs = computed(() =>
	supportedKinds.map((kind) => ({
		label:
			kind === 'textures'
				? formatMessage(messages.textures)
				: kind === 'shaders'
					? formatMessage(messages.shaders)
					: formatMessage(messages.maps),
		href: {
			path: `/browse/bedrock/${kind}`,
			query: {
				i: route.query.i,
			},
		},
	})),
)

const sortTypes = [
	{ display: 'Relevance', name: 'relevance' },
	{ display: 'Title', name: 'title' },
	{ display: 'Version (newest)', name: 'version_desc' },
	{ display: 'Version (oldest)', name: 'version_asc' },
]

function kindDisplayName(kind: string) {
	if (kind === 'textures') return formatMessage(messages.kindTextures)
	if (kind === 'shaders') return formatMessage(messages.kindShaders)
	return formatMessage(messages.kindMaps)
}

async function openItem(item: BedrockItem) {
	await router.push({
		path: `/browse/bedrock/${contentType.value}/item`,
		query: {
			i: route.query.i,
			p: item.page_url,
		},
	})
}

const uniqueSupportedVersions = computed(() => {
	const set = new Set<string>()
	for (const item of pageData.value.items ?? []) {
		for (const version of item.supported_versions ?? []) {
			set.add(version)
		}
	}
	return [...set].sort((a, b) => {
		const pa = a.split('.').map((x) => Number.parseInt(x, 10) || 0)
		const pb = b.split('.').map((x) => Number.parseInt(x, 10) || 0)
		return (pb[0] ?? 0) - (pa[0] ?? 0) || (pb[1] ?? 0) - (pa[1] ?? 0)
	})
})

const filteredItems = computed(() => {
	let items = [...(pageData.value.items ?? [])]

	if (query.value.trim()) {
		const q = query.value.trim().toLowerCase()
		items = items.filter((item) => item.title.toLowerCase().includes(q))
	}

	if (selectedVersion.value) {
		items = items.filter((item) => (item.supported_versions ?? []).includes(selectedVersion.value))
	} else if (selectedVersions.value.length > 0) {
		items = items.filter((item) => {
			const versions = item.supported_versions ?? []
			return selectedVersions.value.some((selected) => versions.includes(selected))
		})
	}

	if (currentSortType.value?.name === 'title') {
		items.sort((a, b) => a.title.localeCompare(b.title, 'ru'))
	} else if (currentSortType.value?.name === 'version_desc') {
		items.sort((a, b) => compareSupportedVersion(b, a))
	} else if (currentSortType.value?.name === 'version_asc') {
		items.sort((a, b) => compareSupportedVersion(a, b))
	}

	return items
})

function compareSupportedVersion(left: BedrockItem, right: BedrockItem) {
	const lv = bestShortVersion(left.supported_versions ?? [])
	const rv = bestShortVersion(right.supported_versions ?? [])
	return compareShortVersion(lv, rv)
}

function bestShortVersion(versions: string[]) {
	return [...versions].sort(compareShortVersion).at(-1) ?? ''
}

function compareShortVersion(left: string, right: string) {
	const lp = left.split('.').map((x) => Number.parseInt(x, 10) || 0)
	const rp = right.split('.').map((x) => Number.parseInt(x, 10) || 0)
	return (lp[0] ?? 0) - (rp[0] ?? 0) || (lp[1] ?? 0) - (rp[1] ?? 0)
}

const paginatedItems = computed(() => {
	const start = (localPage.value - 1) * maxResults.value
	return filteredItems.value.slice(start, start + maxResults.value)
})

const localPageCount = computed(() =>
	Math.max(1, Math.ceil(filteredItems.value.length / maxResults.value)),
)

function toggleVersion(version: string, enabled: boolean) {
	if (enabled) {
		if (!selectedVersions.value.includes(version)) {
			selectedVersions.value = [...selectedVersions.value, version]
		}
	} else {
		selectedVersions.value = selectedVersions.value.filter((x) => x !== version)
	}
}

const breadcrumbs = useBreadcrumbs()
breadcrumbs.setContext({
	name: formatMessage(messages.discoverContent),
	link: route.path,
	query: route.query,
})

async function loadContentPage(page = 1) {
	if (!supportedKinds.includes(contentType.value)) {
		await router.replace({
			path: '/browse/bedrock/textures',
			query: { i: route.query.i },
		})
		return
	}

	loading.value = true
	try {
		sitePage.value = page
		const fetched = await get_bedrock_content(contentType.value, page)
		pageData.value = {
			page: fetched?.page ?? page,
			total_pages: fetched?.total_pages ?? 1,
			has_next: !!fetched?.has_next,
			items: Array.isArray(fetched?.items) ? fetched.items : [],
		}
		localPage.value = 1
	} catch (error) {
		pageData.value = { page, total_pages: 1, has_next: false, items: [] }
		handleError(error)
	} finally {
		loading.value = false
	}
}

async function refreshInstalledState() {
	if (!instancePath.value) {
		installedUrls.value = new Set()
		return
	}
	const rows = await get_installed_bedrock_content(instancePath.value).catch(() => [])
	const urls = (rows ?? [])
		.map((x: any) => (x?.page_url ? String(x.page_url) : ''))
		.filter((x: string) => x.length > 0)
	installedUrls.value = new Set(urls)
	const titleSet = new Set<string>()
	for (const row of rows ?? []) {
		const name = String(row?.name ?? row?.file_name ?? '').toLowerCase()
		if (name) titleSet.add(name.replace(/[^a-z0-9]+/g, ''))
	}
	installedTitleFingerprints.value = titleSet
}

function itemFingerprint(item: BedrockItem) {
	return String(item.title || '')
		.toLowerCase()
		.replace(/[^a-z0-9]+/g, '')
}

function isItemInstalled(item: BedrockItem) {
	return installedUrls.value.has(item.page_url) || installedTitleFingerprints.value.has(itemFingerprint(item))
}

async function installItem(item: BedrockItem) {
	if (!instancePath.value) return
	installing.value = item.page_url
	installProgress.value[item.page_url] = 5
	const ticker = setInterval(() => {
		const current = installProgress.value[item.page_url] ?? 0
		if (current < 90) {
			installProgress.value[item.page_url] = current + 5
		}
	}, 350)
	try {
		await install_bedrock_content(instancePath.value, contentType.value, item.page_url)
		installProgress.value[item.page_url] = 100
		await refreshInstalledState()
		await loadContentPage(sitePage.value)
	} catch (error) {
		handleError(error)
	} finally {
		clearInterval(ticker)
		delete installProgress.value[item.page_url]
		installing.value = ''
	}
}

function currentInstanceVersionShort() {
	const ver = String(instance.value?.game_version ?? '')
	const match = ver.match(/(\d+\.\d+)/)
	return match ? match[1] : ''
}

async function init() {
	if (instancePath.value) {
		instance.value = await getInstance(instancePath.value).catch(() => null)
		if (instance.value?.loader !== 'bedrock') {
			await router.replace({ path: '/browse/mod', query: route.query })
			return
		}
	} else {
		await router.replace({ path: '/library' })
		return
	}
	await loadContentPage(1)
	await refreshInstalledState()
	const short = currentInstanceVersionShort()
	if (short && uniqueSupportedVersions.value.includes(short)) {
		selectedVersions.value = [short]
		selectedVersion.value = short
	}
}

watch(
	() => [route.params.contentType, route.query.i],
	async () => {
		await init()
	},
)

watch([filteredItems, maxResults], () => {
	if (localPage.value > localPageCount.value) {
		localPage.value = localPageCount.value
	}
})

await init()
</script>

<template>
	<Teleport to="#sidebar-teleport-target">
		<div class="border-0 border-b-[1px] p-4 last:border-b-0 border-[--brand-gradient-border] border-solid">
			<DropdownSelect
				v-slot="{ selected }"
				v-model="selectedVersion"
				:name="formatMessage(messages.gameVersion)"
				:options="['', ...uniqueSupportedVersions]"
				:display-name="(option) => (option ? option : formatMessage(messages.anyVersion))"
			>
				<span class="font-semibold text-primary">{{ formatMessage(messages.gameVersion) }}: </span>
				<span class="font-semibold text-secondary">{{
					selected || formatMessage(messages.anyVersion)
				}}</span>
			</DropdownSelect>
		</div>
		<div class="border-0 border-b-[1px] p-4 last:border-b-0 border-[--brand-gradient-border] border-solid">
			<h3 class="text-base m-0">{{ formatMessage(messages.supportedVersions) }}</h3>
		</div>
		<div
			v-for="version in uniqueSupportedVersions"
			:key="`bedrock-version-${version}`"
			class="border-0 border-b-[1px] px-4 py-2 last:border-b-0 border-[--brand-gradient-border] border-solid"
		>
			<Checkbox
				:model-value="selectedVersions.includes(version)"
				:label="version"
				class="filter-checkbox"
				@update:model-value="(value) => toggleVersion(version, value)"
			/>
		</div>
	</Teleport>

	<div ref="searchWrapper" class="flex flex-col gap-3 p-6">
		<template v-if="instance">
			<InstanceIndicator :instance="instance" />
			<h1 class="m-0 mb-1 text-xl">{{ formatMessage(messages.installToInstance) }}</h1>
		</template>

		<NavTabs :links="tabs" />

		<div class="iconified-input">
			<SearchIcon aria-hidden="true" class="text-lg" />
			<input
				v-model="query"
				class="h-12 card-shadow"
				autocomplete="off"
				spellcheck="false"
				type="text"
				:placeholder="formatMessage(messages.searchPlaceholder, { projectType: kindDisplayName(contentType) })"
			/>
			<Button v-if="query" class="r-btn" @click="query = ''">
				<XIcon />
			</Button>
		</div>

		<div class="flex gap-2">
			<DropdownSelect
				v-slot="{ selected }"
				v-model="currentSortType"
				class="max-w-[16rem]"
				:name="formatMessage(messages.sortBy)"
				:options="sortTypes"
				:display-name="(option) => option?.display"
			>
				<span class="font-semibold text-primary">{{ formatMessage(messages.sortBy) }}: </span>
				<span class="font-semibold text-secondary">{{ selected }}</span>
			</DropdownSelect>
			<DropdownSelect
				v-slot="{ selected }"
				v-model="maxResults"
				name="Max results"
				:options="[5, 10, 15, 20, 50, 100]"
				class="max-w-[9rem]"
			>
				<span class="font-semibold text-primary">{{ formatMessage(messages.view) }}: </span>
				<span class="font-semibold text-secondary">{{ selected }}</span>
			</DropdownSelect>
			<Pagination
				:page="sitePage"
				:count="Math.max(1, pageData.total_pages || (pageData.has_next ? sitePage + 1 : sitePage))"
				class="ml-auto"
				@switch-page="(p) => loadContentPage(p)"
			/>
		</div>

		<div v-if="selectedVersions.length > 0" class="flex flex-wrap gap-1 items-center pb-4">
			<span
				v-for="version in selectedVersions"
				:key="`selected-version-${version}`"
				class="text-sm font-semibold text-secondary flex gap-1 px-[0.375rem] py-0.5 bg-button-bg rounded-full"
			>
				{{ version }}
			</span>
		</div>

		<div class="search">
			<section v-if="loading" class="offline">
				<LoadingIndicator />
			</section>
			<section v-else-if="paginatedItems.length === 0" class="offline">
				{{ formatMessage(messages.noResults) }}
			</section>
			<section v-else class="project-list display-mode--list instance-results" role="list">
				<div
					v-for="item in paginatedItems"
					:key="item.page_url"
					class="card-shadow p-4 bg-bg-raised rounded-xl flex gap-3 group cursor-pointer hover:brightness-90 transition-all"
					@click="openItem(item)"
				>
					<div class="icon w-[96px] h-[96px] relative">
						<img
							v-if="item.image_url"
							:src="item.image_url"
							class="w-[96px] h-[96px] rounded-xl object-cover"
							alt="preview"
						/>
					</div>
					<div class="flex flex-col gap-2 overflow-hidden flex-1">
						<div class="gap-2 overflow-hidden no-wrap text-ellipsis">
							<span class="text-lg font-extrabold text-contrast m-0 leading-none">
								{{ item.title }}
							</span>
						</div>
						<div class="m-0 line-clamp-2 text-secondary">
							{{ item.description || kindDisplayName(item.kind) }}
						</div>
						<div v-if="item.supported_versions?.length" class="mt-auto flex items-center gap-1 no-wrap">
							<TagsIcon class="h-4 w-4 shrink-0" />
							<div
								v-for="v in item.supported_versions || []"
								:key="`${item.page_url}-v-${v}`"
								class="text-sm font-semibold text-secondary flex gap-1 px-[0.375rem] py-0.5 bg-button-bg rounded-full"
							>
								{{ v }}
							</div>
						</div>
					</div>
					<div class="flex flex-col gap-2 items-end shrink-0 ml-auto">
						<a
							:href="item.page_url"
							target="_blank"
							rel="noreferrer"
							class="text-secondary text-sm"
							@click.stop
						>
							{{ formatMessage(messages.openSourcePage) }} <ExternalIcon class="inline ml-1" />
						</a>
						<div class="mt-auto relative">
							<div class="absolute bottom-0 right-0 w-fit">
								<ButtonStyled color="brand" type="outlined">
									<button
										:disabled="installing === item.page_url || isItemInstalled(item)"
										class="shrink-0 no-wrap"
										@click.stop="installItem(item)"
									>
										<DownloadIcon />
										{{
											isItemInstalled(item)
												? formatMessage(messages.installed)
												: installing === item.page_url
													? `${formatMessage(messages.installing)} ${Math.max(0, Math.min(100, Math.round(installProgress[item.page_url] || 0)))}%`
													: formatMessage(messages.install)
										}}
									</button>
								</ButtonStyled>
								<div
									v-if="installing === item.page_url"
									class="h-1 rounded-full bg-button-bg mt-2 overflow-hidden"
								>
									<div
										class="h-full bg-brand transition-all duration-300"
										:style="{ width: `${Math.max(0, Math.min(100, installProgress[item.page_url] || 0))}%` }"
									></div>
								</div>
							</div>
						</div>
					</div>
				</div>
			</section>
			<div class="flex justify-end">
				<Pagination
					:page="localPage"
					:count="localPageCount"
					class="pagination-after"
					@switch-page="(p) => (localPage = p)"
				/>
			</div>
		</div>
	</div>
</template>
