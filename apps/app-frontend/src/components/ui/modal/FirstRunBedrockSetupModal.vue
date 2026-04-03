<script setup>
import { ArchiveIcon, BoxIcon, DownloadIcon, SaveIcon } from '@modrinth/assets'
import { Button, defineMessages, injectNotificationManager, useVIntl } from '@modrinth/ui'
import { invoke } from '@tauri-apps/api/core'
import { save } from '@tauri-apps/plugin-dialog'
import { computed, ref } from 'vue'

import ModalWrapper from '@/components/ui/modal/ModalWrapper.vue'
import { get as getSettings, set as setSettings } from '@/helpers/settings'

const { addNotification, handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()

const messages = defineMessages({
	title: {
		id: 'first-run.bedrock.title',
		defaultMessage: 'First start setup',
	},
	description: {
		id: 'first-run.bedrock.description',
		defaultMessage:
			'Before using Bedrock instances, you can back up your current game data and install required runtimes.',
	},
	backupTitle: {
		id: 'first-run.bedrock.backup.title',
		defaultMessage: 'Save Bedrock game files',
	},
	backupText: {
		id: 'first-run.bedrock.backup.text',
		defaultMessage:
			'Creates a .zip backup of com.mojang (worlds, packs, settings) and lets you choose where to save it.',
	},
	backupAction: {
		id: 'first-run.bedrock.backup.action',
		defaultMessage: 'Save game files',
	},
	backupDone: {
		id: 'first-run.bedrock.backup.done',
		defaultMessage: 'Backup was created successfully.',
	},
	backupUnavailable: {
		id: 'first-run.bedrock.backup.unavailable',
		defaultMessage: 'Bedrock data folder was not found on this PC.',
	},
	depsTitle: {
		id: 'first-run.bedrock.deps.title',
		defaultMessage: 'Install required libraries',
	},
	depsText: {
		id: 'first-run.bedrock.deps.text',
		defaultMessage:
			'Installs required runtimes for launcher features (.NET, WebView2, VC++).',
	},
	depsAction: {
		id: 'first-run.bedrock.deps.action',
		defaultMessage: 'Install libraries',
	},
	depsDone: {
		id: 'first-run.bedrock.deps.done',
		defaultMessage: 'Runtime dependency installation finished.',
	},
	allReady: {
		id: 'first-run.bedrock.deps.ready',
		defaultMessage: 'Required libraries are already installed.',
	},
	close: {
		id: 'first-run.bedrock.close',
		defaultMessage: 'Continue',
	},
	working: {
		id: 'first-run.bedrock.working',
		defaultMessage: 'Working...',
	},
})

const modal = ref()
const bedrockDataExists = ref(false)
const dependencyStatus = ref({
	hasWinget: false,
	hasDotnet8: false,
	hasWebview2: false,
	missing: [],
})
const checking = ref(false)
const backingUp = ref(false)
const installingDeps = ref(false)

const missingDeps = computed(() => dependencyStatus.value?.missing ?? [])
const canBackup = computed(() => bedrockDataExists.value && !checking.value && !backingUp.value)
const canInstallDeps = computed(
	() =>
		missingDeps.value.length > 0 &&
		dependencyStatus.value?.hasWinget &&
		!checking.value &&
		!installingDeps.value,
)

async function markOnboarded() {
	const settings = await getSettings()
	if (!settings.onboarded) {
		await setSettings({ ...settings, onboarded: true })
	}
}

async function refreshStatus() {
	checking.value = true
	try {
		bedrockDataExists.value = await invoke('plugin:utils|bedrock_user_data_exists')
		dependencyStatus.value = await invoke('plugin:utils|check_runtime_dependencies')
	} finally {
		checking.value = false
	}
}

async function runBackup() {
	if (!canBackup.value) return
	backingUp.value = true
	try {
		const output = await save({
			defaultPath: 'bedrock-com.mojang-backup.zip',
			filters: [{ name: 'Zip Archive', extensions: ['zip'] }],
		})
		if (!output) return
		await invoke('plugin:utils|backup_bedrock_userdata_zip', { outputPath: output })
		addNotification({
			title: formatMessage(messages.backupDone),
			type: 'success',
		})
	} catch (err) {
		handleError(err)
	} finally {
		backingUp.value = false
	}
}

async function runDependencyInstall() {
	if (!canInstallDeps.value) return
	installingDeps.value = true
	try {
		await invoke('plugin:utils|install_runtime_dependencies')
		await refreshStatus()
		addNotification({
			title: formatMessage(messages.depsDone),
			type: 'success',
		})
	} catch (err) {
		handleError(err)
	} finally {
		installingDeps.value = false
	}
}

async function closeModal() {
	try {
		await markOnboarded()
	} catch (err) {
		handleError(err)
	}
	modal.value?.hide()
}

defineExpose({
	show: async () => {
		await refreshStatus()
		modal.value?.show()
	},
})
</script>

<template>
	<ModalWrapper ref="modal" :header="formatMessage(messages.title)" :on-hide="markOnboarded">
		<div class="first-run-setup">
			<p class="first-run-description">
				{{ formatMessage(messages.description) }}
			</p>

			<div class="first-run-card">
				<div class="first-run-card-header">
					<ArchiveIcon />
					<h3>{{ formatMessage(messages.backupTitle) }}</h3>
				</div>
				<p>{{ formatMessage(messages.backupText) }}</p>
				<p v-if="!bedrockDataExists" class="first-run-muted">
					{{ formatMessage(messages.backupUnavailable) }}
				</p>
				<Button color="primary" :disabled="!canBackup" @click="runBackup">
					<SaveIcon v-if="!backingUp" />
					{{ backingUp ? formatMessage(messages.working) : formatMessage(messages.backupAction) }}
				</Button>
			</div>

			<div class="first-run-card">
				<div class="first-run-card-header">
					<BoxIcon />
					<h3>{{ formatMessage(messages.depsTitle) }}</h3>
				</div>
				<p>{{ formatMessage(messages.depsText) }}</p>
				<p v-if="missingDeps.length === 0" class="first-run-muted">
					{{ formatMessage(messages.allReady) }}
				</p>
				<p v-else class="first-run-muted">
					Missing: {{ missingDeps.join(', ') }}
				</p>
				<p v-if="missingDeps.length > 0 && !dependencyStatus.hasWinget" class="first-run-muted">
					WinGet is not available, so automatic dependency install is disabled.
				</p>
				<Button color="primary" :disabled="!canInstallDeps" @click="runDependencyInstall">
					<DownloadIcon v-if="!installingDeps" />
					{{ installingDeps ? formatMessage(messages.working) : formatMessage(messages.depsAction) }}
				</Button>
			</div>

			<div class="first-run-actions">
				<Button @click="closeModal">
					{{ formatMessage(messages.close) }}
				</Button>
			</div>
		</div>
	</ModalWrapper>
</template>

<style scoped>
.first-run-setup {
	display: flex;
	flex-direction: column;
	gap: var(--gap-md);
}

.first-run-description {
	margin: 0;
	color: var(--color-base);
}

.first-run-card {
	display: flex;
	flex-direction: column;
	gap: var(--gap-sm);
	padding: var(--gap-md);
	border: 1px solid var(--color-divider);
	border-radius: var(--radius-lg);
	background: var(--color-bg);
}

.first-run-card-header {
	display: flex;
	align-items: center;
	gap: var(--gap-sm);
}

.first-run-card-header h3 {
	margin: 0;
	font-size: 1rem;
	font-weight: 700;
}

.first-run-card p {
	margin: 0;
}

.first-run-muted {
	color: var(--color-secondary);
}

.first-run-actions {
	display: flex;
	justify-content: flex-end;
}
</style>
