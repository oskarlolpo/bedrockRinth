<script setup>
import {
	ClipboardCopyIcon,
	EyeIcon,
	FolderOpenIcon,
	PlayIcon,
	PlusIcon,
	SearchIcon,
	StopCircleIcon,
	TrashIcon,
	XIcon,
} from '@modrinth/assets'
import { Button, defineMessages, DropdownSelect, injectNotificationManager, useVIntl } from '@modrinth/ui'
import { formatCategoryHeader } from '@modrinth/utils'
import { useStorage } from '@vueuse/core'
import dayjs from 'dayjs'
import { computed, ref } from 'vue'

import ContextMenu from '@/components/ui/ContextMenu.vue'
import Instance from '@/components/ui/Instance.vue'
import ConfirmModalWrapper from '@/components/ui/modal/ConfirmModalWrapper.vue'
import { duplicate, remove } from '@/helpers/profile.js'

const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()

const props = defineProps({
	instances: {
		type: Array,
		default() {
			return []
		},
	},
	label: {
		type: String,
		default: '',
	},
})

const messages = defineMessages({
	search: {
		id: 'common.search',
		defaultMessage: 'Search',
	},
	sortBy: {
		id: 'common.sort-by',
		defaultMessage: 'Sort by',
	},
	groupBy: {
		id: 'library.group-by',
		defaultMessage: 'Group by',
	},
	select: {
		id: 'common.select',
		defaultMessage: 'Select...',
	},
	sortName: {
		id: 'library.sort.name',
		defaultMessage: 'Name',
	},
	sortLastPlayed: {
		id: 'library.sort.last-played',
		defaultMessage: 'Last played',
	},
	sortDateCreated: {
		id: 'library.sort.date-created',
		defaultMessage: 'Date created',
	},
	sortDateModified: {
		id: 'library.sort.date-modified',
		defaultMessage: 'Date modified',
	},
	sortGameVersion: {
		id: 'library.sort.game-version',
		defaultMessage: 'Game version',
	},
	groupGroup: {
		id: 'library.group.group',
		defaultMessage: 'Group',
	},
	groupLoader: {
		id: 'library.group.loader',
		defaultMessage: 'Loader',
	},
	groupGameVersion: {
		id: 'library.group.game-version',
		defaultMessage: 'Game version',
	},
	groupNone: {
		id: 'library.group.none',
		defaultMessage: 'None',
	},
	deleteInstanceTitle: {
		id: 'library.delete-instance.title',
		defaultMessage: 'Are you sure you want to delete this instance?',
	},
	deleteInstanceDescription: {
		id: 'library.delete-instance.description',
		defaultMessage:
			'If you proceed, all data for your instance will be removed. You will not be able to recover it.',
	},
	delete: {
		id: 'common.delete',
		defaultMessage: 'Delete',
	},
	play: {
		id: 'common.play',
		defaultMessage: 'Play',
	},
	stop: {
		id: 'common.stop',
		defaultMessage: 'Stop',
	},
	addContent: {
		id: 'common.add-content',
		defaultMessage: 'Add content',
	},
	viewInstance: {
		id: 'library.view-instance',
		defaultMessage: 'View instance',
	},
	duplicateInstance: {
		id: 'library.duplicate-instance',
		defaultMessage: 'Duplicate instance',
	},
	openFolder: {
		id: 'common.open-folder',
		defaultMessage: 'Open folder',
	},
	copyPath: {
		id: 'common.copy-path',
		defaultMessage: 'Copy path',
	},
})

const sortOptions = ['Name', 'Last played', 'Date created', 'Date modified', 'Game version']
const groupOptions = ['Group', 'Loader', 'Game version', 'None']

const sortLabelMap = {
	Name: formatMessage(messages.sortName),
	'Last played': formatMessage(messages.sortLastPlayed),
	'Date created': formatMessage(messages.sortDateCreated),
	'Date modified': formatMessage(messages.sortDateModified),
	'Game version': formatMessage(messages.sortGameVersion),
}

const groupLabelMap = {
	Group: formatMessage(messages.groupGroup),
	Loader: formatMessage(messages.groupLoader),
	'Game version': formatMessage(messages.groupGameVersion),
	None: formatMessage(messages.groupNone),
}
const instanceOptions = ref(null)
const instanceComponents = ref(null)

const currentDeleteInstance = ref(null)
const confirmModal = ref(null)

async function deleteProfile() {
	if (currentDeleteInstance.value) {
		instanceComponents.value = instanceComponents.value.filter(
			(x) => x.instance.path !== currentDeleteInstance.value,
		)
		await remove(currentDeleteInstance.value).catch(handleError)
	}
}

async function duplicateProfile(p) {
	await duplicate(p).catch(handleError)
}

const handleRightClick = (event, profilePathId) => {
	const item = instanceComponents.value.find((x) => x.instance.path === profilePathId)
	const baseOptions = [
		{ name: 'add_content' },
		{ type: 'divider' },
		{ name: 'edit' },
		{ name: 'duplicate' },
		{ name: 'open' },
		{ name: 'copy' },
		{ type: 'divider' },
		{
			name: 'delete',
			color: 'danger',
		},
	]

	instanceOptions.value.showMenu(
		event,
		item,
		item.playing
			? [
					{
						name: 'stop',
						color: 'danger',
					},
					...baseOptions,
				]
			: [
					{
						name: 'play',
						color: 'primary',
					},
					...baseOptions,
				],
	)
}

const handleOptionsClick = async (args) => {
	switch (args.option) {
		case 'play':
			args.item.play(null, 'InstanceGridContextMenu')
			break
		case 'stop':
			args.item.stop(null, 'InstanceGridContextMenu')
			break
		case 'add_content':
			await args.item.addContent()
			break
		case 'edit':
			await args.item.seeInstance()
			break
		case 'duplicate':
			if (args.item.instance.install_stage == 'installed')
				await duplicateProfile(args.item.instance.path)
			break
		case 'open':
			await args.item.openFolder()
			break
		case 'copy':
			await navigator.clipboard.writeText(args.item.instance.path)
			break
		case 'delete':
			currentDeleteInstance.value = args.item.instance.path
			confirmModal.value.show()
			break
	}
}

const state = useStorage(
	`${props.label}-grid-display-state`,
	{
		group: 'Group',
		sortBy: 'Name',
	},
	localStorage,
	{ mergeDefaults: true },
)

const search = ref('')

const filteredResults = computed(() => {
	const { group = 'Group', sortBy = 'Name' } = state.value

	const instances = props.instances.filter((instance) => {
		return instance.name.toLowerCase().includes(search.value.toLowerCase())
	})

	if (sortBy === 'Name') {
		instances.sort((a, b) => {
			return a.name.localeCompare(b.name)
		})
	}

	if (sortBy === 'Game version') {
		instances.sort((a, b) => {
			return a.game_version.localeCompare(b.game_version, undefined, { numeric: true })
		})
	}

	if (sortBy === 'Last played') {
		instances.sort((a, b) => {
			return dayjs(b.last_played ?? 0).diff(dayjs(a.last_played ?? 0))
		})
	}

	if (sortBy === 'Date created') {
		instances.sort((a, b) => {
			return dayjs(b.date_created).diff(dayjs(a.date_created))
		})
	}

	if (sortBy === 'Date modified') {
		instances.sort((a, b) => {
			return dayjs(b.date_modified).diff(dayjs(a.date_modified))
		})
	}

	const instanceMap = new Map()

	if (group === 'Loader') {
		instances.forEach((instance) => {
			const loader = formatCategoryHeader(instance.loader)
			if (!instanceMap.has(loader)) {
				instanceMap.set(loader, [])
			}

			instanceMap.get(loader).push(instance)
		})
	} else if (group === 'Game version') {
		instances.forEach((instance) => {
			if (!instanceMap.has(instance.game_version)) {
				instanceMap.set(instance.game_version, [])
			}

			instanceMap.get(instance.game_version).push(instance)
		})
	} else if (group === 'Group') {
		instances.forEach((instance) => {
			if (instance.groups.length === 0) {
				instance.groups.push('None')
			}

			for (const category of instance.groups) {
				if (!instanceMap.has(category)) {
					instanceMap.set(category, [])
				}

				instanceMap.get(category).push(instance)
			}
		})
	} else {
		return instanceMap.set('None', instances)
	}

	// For 'name', we intuitively expect the sorting to apply to the name of the group first, not just the name of the instance
	// ie: Category A should come before B, even if the first instance in B comes before the first instance in A
	if (sortBy === 'Name') {
		const sortedEntries = [...instanceMap.entries()].sort((a, b) => {
			// None should always be first
			if (a[0] === 'None' && b[0] !== 'None') {
				return -1
			}
			if (a[0] !== 'None' && b[0] === 'None') {
				return 1
			}
			return a[0].localeCompare(b[0])
		})
		instanceMap.clear()
		sortedEntries.forEach((entry) => {
			instanceMap.set(entry[0], entry[1])
		})
	}
	// default sorting would do 1.20.4 < 1.8.9 because 2 < 8
	// localeCompare with numeric=true puts 1.8.9 < 1.20.4 because 8 < 20
	if (group === 'Game version') {
		const sortedEntries = [...instanceMap.entries()].sort((a, b) => {
			return a[0].localeCompare(b[0], undefined, { numeric: true })
		})
		instanceMap.clear()
		sortedEntries.forEach((entry) => {
			instanceMap.set(entry[0], entry[1])
		})
	}

	return instanceMap
})
</script>
<template>
	<div class="flex gap-2">
		<div class="iconified-input flex-1">
			<SearchIcon />
			<input v-model="search" type="text" :placeholder="formatMessage(messages.search)" />
			<Button class="r-btn" @click="() => (search = '')">
				<XIcon />
			</Button>
		</div>
		<DropdownSelect
			v-slot="{ selected }"
			v-model="state.sortBy"
			name="Sort Dropdown"
			class="max-w-[16rem]"
			:options="sortOptions"
			:placeholder="formatMessage(messages.select)"
			:display-name="(option) => sortLabelMap[option] ?? option"
		>
			<span class="font-semibold text-primary">{{ formatMessage(messages.sortBy) }}: </span>
			<span class="font-semibold text-secondary">{{ sortLabelMap[selected] ?? selected }}</span>
		</DropdownSelect>
		<DropdownSelect
			v-slot="{ selected }"
			v-model="state.group"
			class="max-w-[16rem]"
			name="Group Dropdown"
			:options="groupOptions"
			:placeholder="formatMessage(messages.select)"
			:display-name="(option) => groupLabelMap[option] ?? option"
		>
			<span class="font-semibold text-primary">{{ formatMessage(messages.groupBy) }}: </span>
			<span class="font-semibold text-secondary">{{ groupLabelMap[selected] ?? selected }}</span>
		</DropdownSelect>
	</div>
	<div
		v-for="instanceSection in Array.from(filteredResults, ([key, value]) => ({
			key,
			value,
		}))"
		:key="instanceSection.key"
		class="row"
	>
		<div v-if="instanceSection.key !== 'None'" class="divider">
			<p>{{ instanceSection.key }}</p>
			<hr aria-hidden="true" />
		</div>
		<section class="instances">
			<Instance
				v-for="instance in instanceSection.value"
				ref="instanceComponents"
				:key="instance.path + instance.install_stage"
				:instance="instance"
				@contextmenu.prevent.stop="(event) => handleRightClick(event, instance.path)"
			/>
		</section>
	</div>
	<ConfirmModalWrapper
		ref="confirmModal"
		:title="formatMessage(messages.deleteInstanceTitle)"
		:description="formatMessage(messages.deleteInstanceDescription)"
		:has-to-type="false"
		:proceed-label="formatMessage(messages.delete)"
		@proceed="deleteProfile"
	/>
	<ContextMenu ref="instanceOptions" @option-clicked="handleOptionsClick">
		<template #play> <PlayIcon /> {{ formatMessage(messages.play) }} </template>
		<template #stop> <StopCircleIcon /> {{ formatMessage(messages.stop) }} </template>
		<template #add_content> <PlusIcon /> {{ formatMessage(messages.addContent) }} </template>
		<template #edit> <EyeIcon /> {{ formatMessage(messages.viewInstance) }} </template>
		<template #duplicate>
			<ClipboardCopyIcon /> {{ formatMessage(messages.duplicateInstance) }}
		</template>
		<template #delete> <TrashIcon /> {{ formatMessage(messages.delete) }} </template>
		<template #open> <FolderOpenIcon /> {{ formatMessage(messages.openFolder) }} </template>
		<template #copy> <ClipboardCopyIcon /> {{ formatMessage(messages.copyPath) }} </template>
	</ContextMenu>
</template>
<style lang="scss" scoped>
.row {
	display: flex;
	flex-direction: column;
	align-items: flex-start;
	width: 100%;

	.divider {
		display: flex;
		justify-content: space-between;
		align-items: center;
		width: 100%;
		gap: 1rem;
		margin-bottom: 1rem;

		p {
			margin: 0;
			font-size: 1rem;
			white-space: nowrap;
			color: var(--color-contrast);
		}

		hr {
			background-color: var(--color-gray);
			height: 1px;
			width: 100%;
			border: none;
		}
	}
}

.header {
	display: flex;
	flex-direction: row;
	flex-wrap: wrap;
	justify-content: space-between;
	gap: 1rem;
	align-items: inherit;
	margin: 1rem 1rem 0 !important;
	padding: 1rem;
	width: calc(100% - 2rem);

	.iconified-input {
		flex-grow: 1;

		input {
			min-width: 100%;
		}
	}

	.sort-dropdown {
		width: 10rem;
	}

	.filter-dropdown {
		width: 15rem;
	}

	.group-dropdown {
		width: 10rem;
	}

	.labeled_button {
		display: flex;
		flex-direction: row;
		align-items: center;
		gap: 0.5rem;
		white-space: nowrap;
	}
}

.instances {
	display: grid;
	grid-template-columns: repeat(auto-fill, minmax(16rem, 1fr));
	width: 100%;
	gap: 0.75rem;
	margin-right: auto;
	scroll-behavior: smooth;
	overflow-y: auto;
}
</style>
