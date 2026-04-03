<template>
	<ModalWrapper ref="modal" :header="formatMessage(messages.header)">
		<div class="modal-header">
			<Chips
				v-model="creationType"
				:items="['custom', 'from file', 'import from launcher']"
				:format-label="formatCreationType"
			/>
		</div>
		<hr class="card-divider" />
		<div v-if="creationType === 'custom'" class="modal-body">
			<div class="image-upload">
				<Avatar :src="display_icon" size="md" :rounded="true" />
				<div class="image-input">
					<Button @click="upload_icon()">
						<UploadIcon />
						{{ formatMessage(messages.selectIcon) }}
					</Button>
					<Button :disabled="!display_icon" @click="reset_icon">
						<XIcon />
						{{ formatMessage(messages.removeIcon) }}
					</Button>
				</div>
			</div>
			<div class="input-row">
				<p class="input-label">{{ formatMessage(messages.name) }}</p>
				<input
					v-model="profile_name"
					autocomplete="off"
					class="text-input"
					type="text"
					maxlength="100"
				/>
			</div>
			<div class="input-row">
				<p class="input-label">{{ formatMessage(messages.loader) }}</p>
				<Chips v-model="loader" :items="loaders" />
			</div>
			<div class="input-row">
				<p class="input-label">{{ formatMessage(messages.gameVersion) }}</p>
				<div class="flex gap-4 items-center">
					<multiselect
						v-model="game_version"
						class="selector"
						:options="game_versions"
						:multiple="false"
						:searchable="true"
						:placeholder="formatMessage(messages.selectGameVersion)"
						open-direction="top"
						:show-labels="false"
					/>
					<Checkbox
						v-model="showSnapshots"
						class="shrink-0"
						:label="formatMessage(messages.showAllVersions)"
					/>
				</div>
			</div>
			<div v-if="loader !== 'vanilla' && loader !== 'bedrock'" class="input-row">
				<p class="input-label">{{ formatMessage(messages.loaderVersion) }}</p>
				<Chips
					v-model="loader_version"
					:items="['stable', 'latest', 'other']"
					:format-label="formatLoaderVersion"
				/>
			</div>
			<div v-if="loader_version === 'other' && loader !== 'vanilla' && loader !== 'bedrock'">
				<div v-if="game_version" class="input-row">
					<p class="input-label">{{ formatMessage(messages.selectVersion) }}</p>
					<multiselect
						v-model="specified_loader_version"
						class="selector"
						:options="selectable_versions"
						:searchable="true"
						:placeholder="formatMessage(messages.selectLoaderVersion)"
						open-direction="top"
						:show-labels="false"
					/>
				</div>
				<div v-else class="input-row">
					<p class="warning">
						{{ formatMessage(messages.selectGameBeforeLoaderVersion) }}
					</p>
				</div>
			</div>
			<div class="input-group push-right">
				<Button @click="hide()">
					<XIcon />
					{{ formatMessage(messages.cancel) }}
				</Button>
				<Button color="primary" :disabled="!check_valid || creating" @click="create_instance()">
					<PlusIcon v-if="!creating" />
					{{ creating ? formatMessage(messages.creating) : formatMessage(messages.create) }}
				</Button>
			</div>
		</div>
		<div v-else-if="creationType === 'from file'" class="modal-body">
			<Button @click="openFile">
				<FolderOpenIcon />
				{{ formatMessage(messages.importFromFile) }}
			</Button>
			<div class="info">
				<InfoIcon />
				{{ formatMessage(messages.dragAndDropMrpack) }}
			</div>
		</div>
		<div v-else class="modal-body">
			<Chips
				v-model="selectedProfileType"
				:items="profileOptions"
				:format-label="(profile) => profile?.name"
			/>
			<div class="path-selection">
				<h3>{{ formatMessage(messages.launcherPath, { name: selectedProfileType.name }) }}</h3>
				<div class="path-input">
					<div class="iconified-input">
						<FolderOpenIcon />
						<input
							v-model="selectedProfileType.path"
							type="text"
							:placeholder="formatMessage(messages.pathToLauncher)"
							@change="setPath"
						/>
						<Button class="r-btn" @click="() => (selectedProfileType.path = '')">
							<XIcon />
						</Button>
					</div>
					<Button icon-only @click="selectLauncherPath">
						<FolderSearchIcon />
					</Button>
					<Button icon-only @click="reload">
						<UpdatedIcon />
					</Button>
				</div>
			</div>
			<div class="table">
				<div class="table-head table-row">
					<div class="toggle-all table-cell">
						<Checkbox
							class="select-checkbox"
							:model-value="
								profiles.get(selectedProfileType.name)?.every((child) => child.selected)
							"
							@update:model-value="
								(newValue) =>
									profiles
										.get(selectedProfileType.name)
										?.forEach((child) => (child.selected = newValue))
							"
						/>
					</div>
					<div class="name-cell table-cell">{{ formatMessage(messages.profileName) }}</div>
				</div>
				<div
					v-if="
						profiles.get(selectedProfileType.name) &&
						profiles.get(selectedProfileType.name).length > 0
					"
					class="table-content"
				>
					<div
						v-for="(profile, index) in profiles.get(selectedProfileType.name)"
						:key="index"
						class="table-row"
					>
						<div class="checkbox-cell table-cell">
							<Checkbox v-model="profile.selected" class="select-checkbox" />
						</div>
						<div class="name-cell table-cell">
							{{ profile.name }}
						</div>
					</div>
				</div>
				<div v-else class="table-content empty">{{ formatMessage(messages.noProfilesFound) }}</div>
			</div>
			<div class="button-row">
				<Button
					:disabled="
						loading ||
						!Array.from(profiles.values())
							.flatMap((e) => e)
							.some((e) => e.selected)
					"
					color="primary"
					@click="next"
				>
					{{
						loading
							? formatMessage(messages.importing)
							: Array.from(profiles.values())
										.flatMap((e) => e)
										.some((e) => e.selected)
								? formatMessage(messages.importProfiles, {
										count: Array.from(profiles.values())
											.flatMap((e) => e)
											.filter((e) => e.selected).length,
									})
								: formatMessage(messages.selectProfilesToImport)
					}}
				</Button>
				<ProgressBar
					v-if="loading"
					:progress="(importedProfiles / (totalProfiles + 0.0001)) * 100"
				/>
			</div>
		</div>
	</ModalWrapper>
</template>

<script setup>
import {
	FolderOpenIcon,
	FolderSearchIcon,
	InfoIcon,
	PlusIcon,
	UpdatedIcon,
	UploadIcon,
	XIcon,
} from '@modrinth/assets'
import {
	Avatar,
	Button,
	Checkbox,
	Chips,
	defineMessages,
	injectNotificationManager,
	useVIntl,
} from '@modrinth/ui'
import { convertFileSrc } from '@tauri-apps/api/core'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { open } from '@tauri-apps/plugin-dialog'
import { computed, onUnmounted, ref, shallowRef, watch } from 'vue'
import Multiselect from 'vue-multiselect'

import ModalWrapper from '@/components/ui/modal/ModalWrapper.vue'
import ProgressBar from '@/components/ui/ProgressBar.vue'
import { trackEvent } from '@/helpers/analytics'
import {
	get_default_launcher_path,
	get_importable_instances,
	import_instance,
} from '@/helpers/import.js'
import { get_bedrock_versions, get_game_versions, get_loader_versions } from '@/helpers/metadata'
import {
	create_bedrock_profile_from_package_file,
	create_profile_and_install_from_file,
} from '@/helpers/pack.js'
import { create } from '@/helpers/profile'
import { get_loaders } from '@/helpers/tags'

const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const messages = defineMessages({
	header: {
		id: 'instance.create.header',
		defaultMessage: 'Creating an instance',
	},
	selectIcon: {
		id: 'instance.create.select-icon',
		defaultMessage: 'Select icon',
	},
	removeIcon: {
		id: 'instance.create.remove-icon',
		defaultMessage: 'Remove icon',
	},
	name: {
		id: 'instance.create.name',
		defaultMessage: 'Name',
	},
	loader: {
		id: 'instance.create.loader',
		defaultMessage: 'Loader',
	},
	gameVersion: {
		id: 'instance.create.game-version',
		defaultMessage: 'Game version',
	},
	selectGameVersion: {
		id: 'instance.create.select-game-version',
		defaultMessage: 'Select game version',
	},
	showAllVersions: {
		id: 'instance.create.show-all-versions',
		defaultMessage: 'Show all versions',
	},
	loaderVersion: {
		id: 'instance.create.loader-version',
		defaultMessage: 'Loader version',
	},
	selectVersion: {
		id: 'instance.create.select-version',
		defaultMessage: 'Select version',
	},
	selectLoaderVersion: {
		id: 'instance.create.select-loader-version',
		defaultMessage: 'Select loader version',
	},
	selectGameBeforeLoaderVersion: {
		id: 'instance.create.select-game-before-loader',
		defaultMessage: 'Select a game version before you select a loader version',
	},
	cancel: {
		id: 'instance.create.cancel',
		defaultMessage: 'Cancel',
	},
	create: {
		id: 'instance.create.create',
		defaultMessage: 'Create',
	},
	creating: {
		id: 'instance.create.creating',
		defaultMessage: 'Creating...',
	},
	importFromFile: {
		id: 'instance.create.import-from-file',
		defaultMessage: 'Import from file',
	},
	dragAndDropMrpack: {
		id: 'instance.create.drag-and-drop',
		defaultMessage: 'Or drag and drop your .mrpack/.appx/.msix/.msixvc file',
	},
	launcherPath: {
		id: 'instance.create.launcher-path',
		defaultMessage: '{name} path',
	},
	pathToLauncher: {
		id: 'instance.create.path-to-launcher',
		defaultMessage: 'Path to launcher',
	},
	profileName: {
		id: 'instance.create.profile-name',
		defaultMessage: 'Profile name',
	},
	noProfilesFound: {
		id: 'instance.create.no-profiles-found',
		defaultMessage: 'No profiles found',
	},
	importing: {
		id: 'instance.create.importing',
		defaultMessage: 'Importing...',
	},
	importProfiles: {
		id: 'instance.create.import-profiles',
		defaultMessage: 'Import {count} profiles',
	},
	selectProfilesToImport: {
		id: 'instance.create.select-profiles',
		defaultMessage: 'Select profiles to import',
	},
	creationCustom: {
		id: 'instance.create.mode.custom',
		defaultMessage: 'custom',
	},
	creationFromFile: {
		id: 'instance.create.mode.from-file',
		defaultMessage: 'from file',
	},
	creationFromLauncher: {
		id: 'instance.create.mode.import-launcher',
		defaultMessage: 'import from launcher',
	},
	loaderStable: {
		id: 'instance.create.loader-version.stable',
		defaultMessage: 'stable',
	},
	loaderLatest: {
		id: 'instance.create.loader-version.latest',
		defaultMessage: 'latest',
	},
	loaderOther: {
		id: 'instance.create.loader-version.other',
		defaultMessage: 'other',
	},
})

const profile_name = ref('')
const game_version = ref('')
const loader = ref('vanilla')
const loader_version = ref('stable')
const specified_loader_version = ref('')
const icon = ref(null)
const display_icon = ref(null)
const creating = ref(false)
const showSnapshots = ref(false)
const creationType = ref('custom')
const isShowing = ref(false)

const formatCreationType = (value) => {
	if (value === 'custom') return formatMessage(messages.creationCustom)
	if (value === 'from file') return formatMessage(messages.creationFromFile)
	if (value === 'import from launcher') return formatMessage(messages.creationFromLauncher)
	return value
}

const formatLoaderVersion = (value) => {
	if (value === 'stable') return formatMessage(messages.loaderStable)
	if (value === 'latest') return formatMessage(messages.loaderLatest)
	if (value === 'other') return formatMessage(messages.loaderOther)
	return value
}

defineExpose({
	show: async () => {
		game_version.value = ''
		specified_loader_version.value = ''
		profile_name.value = ''
		creating.value = false
		showSnapshots.value = false
		loader.value = 'vanilla'
		loader_version.value = 'stable'
		icon.value = null
		display_icon.value = null
		isShowing.value = true
		modal.value.show()

		unlistener.value = await getCurrentWebview().onDragDropEvent(async (event) => {
			// Only if modal is showing
			if (!isShowing.value) return
			if (event.payload.type !== 'drop') return
			if (creationType.value !== 'from file') return
			hide()
			const { paths } = event.payload
			if (paths && paths.length > 0) {
				await importProfileFromPath(paths[0]).catch(handleError)
			}
		})

		trackEvent('InstanceCreateStart', { source: 'CreationModal' })
	},
})

const unlistener = ref(null)
const hide = () => {
	isShowing.value = false
	modal.value.hide()
	if (unlistener.value) {
		unlistener.value()
		unlistener.value = null
	}
}
onUnmounted(() => {
	if (unlistener.value) {
		unlistener.value()
		unlistener.value = null
	}
})

const [
	fabric_versions,
	forge_versions,
	quilt_versions,
	neoforge_versions,
	all_game_versions,
	loaders,
] = await Promise.all([
	get_loader_versions('fabric').then(shallowRef).catch(handleError),
	get_loader_versions('forge').then(shallowRef).catch(handleError),
	get_loader_versions('quilt').then(shallowRef).catch(handleError),
	get_loader_versions('neo').then(shallowRef).catch(handleError),
	get_game_versions().then(shallowRef).catch(handleError),
	get_loaders()
		.then((value) =>
			ref(
				value
					.filter((item) => item.supported_project_types.includes('modpack'))
					.map((item) => item.name.toLowerCase()),
			),
		)
		.catch((err) => {
			handleError(err)
			return ref([])
		}),
])
loaders.value.unshift('vanilla')
if (!loaders.value.includes('bedrock')) {
	loaders.value.push('bedrock')
}
const bedrock_versions = shallowRef([])

const reloadBedrockVersions = async () => {
	bedrock_versions.value = await get_bedrock_versions().catch((err) => {
		handleError(err)
		return []
	})
}

watch(loader, async (value) => {
	if (value === 'bedrock') {
		await reloadBedrockVersions()
	}
})

const game_versions = computed(() => {
	if (loader.value === 'bedrock') {
		return [...new Set((bedrock_versions.value ?? []).map((item) => item?.[0]).filter(Boolean))]
	}

	return all_game_versions.value.versions
		.filter((item) => {
			let defaultVal = item.type === 'release' || showSnapshots.value
			if (loader.value === 'fabric') {
				defaultVal &= fabric_versions.value.gameVersions.some((x) => item.id === x.id)
			} else if (loader.value === 'forge') {
				defaultVal &= forge_versions.value.gameVersions.some((x) => item.id === x.id)
			} else if (loader.value === 'quilt') {
				defaultVal &= quilt_versions.value.gameVersions.some((x) => item.id === x.id)
			} else if (loader.value === 'neoforge') {
				defaultVal &= neoforge_versions.value.gameVersions.some((x) => item.id === x.id)
			}

			return defaultVal
		})
		.map((item) => item.id)
})

const selected_bedrock_version = computed(() => {
	if (loader.value !== 'bedrock') {
		return null
	}

	return (bedrock_versions.value ?? []).find((item) => item?.[0] === game_version.value) ?? null
})

const modal = ref(null)

const check_valid = computed(() => {
	return (
		profile_name.value.trim() &&
		game_version.value &&
		game_versions.value.includes(game_version.value)
	)
})

const create_instance = async () => {
	creating.value = true
	const loader_version_value =
		loader_version.value === 'other' ? specified_loader_version.value : loader_version.value
	const loaderVersion =
		loader.value === 'bedrock'
			? selected_bedrock_version.value
				? `${selected_bedrock_version.value[1]}_${selected_bedrock_version.value[2]}`
				: null
			: loader.value === 'vanilla'
				? null
				: (loader_version_value ?? 'stable')

	hide()
	creating.value = false

	await create(
		profile_name.value,
		game_version.value,
		loader.value,
		loaderVersion,
		icon.value,
	).catch(handleError)

	trackEvent('InstanceCreate', {
		profile_name: profile_name.value,
		game_version: game_version.value,
		loader: loader.value,
		loader_version: loaderVersion,
		has_icon: !!icon.value,
		source: 'CreationModal',
	})
}

const upload_icon = async () => {
	const res = await open({
		multiple: false,
		filters: [
			{
				name: 'Image',
				extensions: ['png', 'jpeg', 'svg', 'webp', 'gif', 'jpg'],
			},
		],
	})

	icon.value = res.path ?? res

	if (!icon.value) return
	display_icon.value = convertFileSrc(icon.value)
}

const reset_icon = () => {
	icon.value = null
	display_icon.value = null
}

const selectable_versions = computed(() => {
	if (game_version.value) {
		if (loader.value === 'fabric') {
			return fabric_versions.value.gameVersions[0].loaders.map((item) => item.id)
		} else if (loader.value === 'forge') {
			return forge_versions.value.gameVersions
				.find((item) => item.id === game_version.value)
				.loaders.map((item) => item.id)
		} else if (loader.value === 'quilt') {
			return quilt_versions.value.gameVersions[0].loaders.map((item) => item.id)
		} else if (loader.value === 'neoforge') {
			return neoforge_versions.value.gameVersions
				.find((item) => item.id === game_version.value)
				.loaders.map((item) => item.id)
		}
	}
	return []
})

const openFile = async () => {
	const newProject = await open({
		multiple: false,
		filters: [
			{
				name: 'Modpack/Bedrock package',
				extensions: ['mrpack', 'appx', 'msix', 'appxbundle', 'msixbundle', 'msixvc'],
			},
		],
	})
	if (!newProject) return
	hide()
	await importProfileFromPath(newProject.path ?? newProject).catch(handleError)
}

const isMrpackPath = (path) => path.toLowerCase().endsWith('.mrpack')
const isBedrockPackagePath = (path) => {
	const normalized = path.toLowerCase()
	return (
		normalized.endsWith('.appx') ||
		normalized.endsWith('.msix') ||
		normalized.endsWith('.appxbundle') ||
		normalized.endsWith('.msixbundle') ||
		normalized.endsWith('.msixvc')
	)
}

const importProfileFromPath = async (path) => {
	const filePath = `${path}`
	if (isMrpackPath(filePath)) {
		await create_profile_and_install_from_file(filePath)
		trackEvent('InstanceCreate', {
			source: 'CreationModalFileImportMrpack',
		})
		return
	}
	if (isBedrockPackagePath(filePath)) {
		await create_bedrock_profile_from_package_file(filePath)
		trackEvent('InstanceCreate', {
			source: 'CreationModalFileImportBedrockPackage',
		})
		return
	}
	throw new Error(`Unsupported import file type: ${filePath}`)
}

const profiles = ref(
	new Map([
		['MultiMC', []],
		['GDLauncher', []],
		['ATLauncher', []],
		['Curseforge', []],
		['PrismLauncher', []],
	]),
)

const loading = ref(false)
const importedProfiles = ref(0)
const totalProfiles = ref(0)

const selectedProfileType = ref('MultiMC')
const profileOptions = ref([
	{ name: 'MultiMC', path: '' },
	{ name: 'GDLauncher', path: '' },
	{ name: 'ATLauncher', path: '' },
	{ name: 'Curseforge', path: '' },
	{ name: 'PrismLauncher', path: '' },
])

// Attempt to get import profiles on default paths
const promises = profileOptions.value.map(async (option) => {
	const path = await get_default_launcher_path(option.name).catch(handleError)
	if (!path || path === '') return

	// Try catch to allow failure and simply ignore default path attempt
	try {
		const instances = await get_importable_instances(option.name, path)

		if (!instances) return
		profileOptions.value.find((profile) => profile.name === option.name).path = path
		profiles.value.set(
			option.name,
			instances.map((name) => ({ name, selected: false })),
		)
	} catch {
		// Allow failure silently
	}
})
await Promise.all(promises)

const selectLauncherPath = async () => {
	selectedProfileType.value.path = await open({ multiple: false, directory: true })

	if (selectedProfileType.value.path) {
		await reload()
	}
}

const reload = async () => {
	const instances = await get_importable_instances(
		selectedProfileType.value.name,
		selectedProfileType.value.path,
	).catch(handleError)
	if (instances) {
		profiles.value.set(
			selectedProfileType.value.name,
			instances.map((name) => ({ name, selected: false })),
		)
	} else {
		profiles.value.set(selectedProfileType.value.name, [])
	}
}

const setPath = () => {
	profileOptions.value.find((profile) => profile.name === selectedProfileType.value.name).path =
		selectedProfileType.value.path
}

const next = async () => {
	importedProfiles.value = 0
	totalProfiles.value = Array.from(profiles.value.values())
		.map((profiles) => profiles.filter((profile) => profile.selected).length)
		.reduce((a, b) => a + b, 0)
	loading.value = true
	for (const launcher of Array.from(profiles.value.entries()).map(([launcher, profiles]) => ({
		launcher,
		path: profileOptions.value.find((option) => option.name === launcher).path,
		profiles,
	}))) {
		for (const profile of launcher.profiles.filter((profile) => profile.selected)) {
			await import_instance(launcher.launcher, launcher.path, profile.name)
				.catch(handleError)
				.then(() => console.log(`Successfully Imported ${profile.name} from ${launcher.launcher}`))
			profile.selected = false
			importedProfiles.value++
		}
	}
	loading.value = false
}
</script>

<style lang="scss" scoped>
.modal-body {
	display: flex;
	flex-direction: column;
	gap: var(--gap-md);
	margin-top: var(--gap-lg);
}

.input-label {
	font-size: 1rem;
	font-weight: bolder;
	color: var(--color-contrast);
	margin-bottom: 0.5rem;
}

.text-input {
	width: 20rem;
}

.image-upload {
	display: flex;
	gap: 1rem;
}

.image-input {
	display: flex;
	flex-direction: column;
	gap: 0.5rem;
	justify-content: center;
}

.warning {
	font-style: italic;
}

:deep(button.checkbox) {
	border: none;
}

.selector {
	max-width: 20rem;
}

.labeled-divider {
	text-align: center;
}

.labeled-divider:after {
	background-color: var(--color-raised-bg);
	content: 'Or';
	color: var(--color-base);
	padding: var(--gap-sm);
	position: relative;
	top: -0.5rem;
}

.info {
	display: flex;
	flex-direction: row;
	gap: 0.5rem;
	align-items: center;
}

.modal-header {
	display: flex;
	flex-direction: row;
	justify-content: space-between;
	align-items: center;
	padding-bottom: 0;
}

.path-selection {
	padding: var(--gap-xl);
	background-color: var(--color-bg);
	border-radius: var(--radius-lg);
	display: flex;
	flex-direction: column;
	gap: var(--gap-md);

	h3 {
		margin: 0;
	}

	.path-input {
		display: flex;
		align-items: center;
		width: 100%;
		flex-direction: row;
		gap: var(--gap-sm);

		.iconified-input {
			flex-grow: 1;
			:deep(input) {
				width: 100%;
				flex-basis: auto;
			}
		}
	}
}

.table {
	border: 1px solid var(--color-bg);
}

.table-row {
	grid-template-columns: min-content auto;
}

.table-content {
	max-height: calc(5 * (18px + 2rem));
	height: calc(5 * (18px + 2rem));
	overflow-y: auto;
}

.select-checkbox {
	button.checkbox {
		border: none;
	}
}

.button-row {
	display: flex;
	flex-direction: row;
	justify-content: space-between;
	align-items: center;
	gap: var(--gap-md);

	.transparent {
		padding: var(--gap-sm) 0;
	}
}

.empty {
	display: flex;
	align-items: center;
	justify-content: center;
	font-size: 1.5rem;
	font-weight: bolder;
	color: var(--color-contrast);
}

.card-divider {
	margin: var(--gap-md) var(--gap-lg) 0 var(--gap-lg);
}
</style>
