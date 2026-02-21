<script setup lang="ts">
import { defineMessages, injectNotificationManager, Slider, Toggle, useVIntl } from '@modrinth/ui'
import { ref, watch } from 'vue'

import useMemorySlider from '@/composables/useMemorySlider'
import { get, set } from '@/helpers/settings.ts'

const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()

const fetchSettings = await get()
fetchSettings.launchArgs = fetchSettings.extra_launch_args.join(' ')
fetchSettings.envVars = fetchSettings.custom_env_vars.map((x) => x.join('=')).join(' ')

const settings = ref(fetchSettings)

const { maxMemory, snapPoints } = (await useMemorySlider().catch(handleError)) as unknown as {
	maxMemory: number
	snapPoints: number[]
}

const messages = defineMessages({
	windowSizeTitle: { id: 'app.settings.default.window-size.title', defaultMessage: 'Window size' },
	fullscreenTitle: { id: 'app.settings.default.window-size.fullscreen.title', defaultMessage: 'Fullscreen' },
	fullscreenDescription: {
		id: 'app.settings.default.window-size.fullscreen.description',
		defaultMessage: 'Overwrites the options.txt file to start in full screen when launched.',
	},
	widthTitle: { id: 'app.settings.default.window-size.width.title', defaultMessage: 'Width' },
	widthDescription: {
		id: 'app.settings.default.window-size.width.description',
		defaultMessage: 'The width of the game window when launched.',
	},
	widthPlaceholder: { id: 'app.settings.default.window-size.width.placeholder', defaultMessage: 'Enter width...' },
	heightTitle: { id: 'app.settings.default.window-size.height.title', defaultMessage: 'Height' },
	heightDescription: {
		id: 'app.settings.default.window-size.height.description',
		defaultMessage: 'The height of the game window when launched.',
	},
	heightPlaceholder: {
		id: 'app.settings.default.window-size.height.placeholder',
		defaultMessage: 'Enter height...',
	},
	memoryTitle: { id: 'app.settings.default.memory.title', defaultMessage: 'Memory allocated' },
	memoryDescription: {
		id: 'app.settings.default.memory.description',
		defaultMessage: 'The memory allocated to each instance when it is ran.',
	},
	javaArgsTitle: { id: 'app.settings.default.java-args.title', defaultMessage: 'Java arguments' },
	javaArgsPlaceholder: {
		id: 'app.settings.default.java-args.placeholder',
		defaultMessage: 'Enter java arguments...',
	},
	envVarsTitle: {
		id: 'app.settings.default.environment.title',
		defaultMessage: 'Environmental variables',
	},
	envVarsPlaceholder: {
		id: 'app.settings.default.environment.placeholder',
		defaultMessage: 'Enter environmental variables...',
	},
	hooksTitle: { id: 'app.settings.default.hooks.title', defaultMessage: 'Hooks' },
	preLaunchTitle: { id: 'app.settings.default.hooks.pre-launch.title', defaultMessage: 'Pre launch' },
	preLaunchDescription: {
		id: 'app.settings.default.hooks.pre-launch.description',
		defaultMessage: 'Ran before the instance is launched.',
	},
	preLaunchPlaceholder: {
		id: 'app.settings.default.hooks.pre-launch.placeholder',
		defaultMessage: 'Enter pre-launch command...',
	},
	wrapperTitle: { id: 'app.settings.default.hooks.wrapper.title', defaultMessage: 'Wrapper' },
	wrapperDescription: {
		id: 'app.settings.default.hooks.wrapper.description',
		defaultMessage: 'Wrapper command for launching Minecraft.',
	},
	wrapperPlaceholder: {
		id: 'app.settings.default.hooks.wrapper.placeholder',
		defaultMessage: 'Enter wrapper command...',
	},
	postExitTitle: { id: 'app.settings.default.hooks.post-exit.title', defaultMessage: 'Post exit' },
	postExitDescription: {
		id: 'app.settings.default.hooks.post-exit.description',
		defaultMessage: 'Ran after the game closes.',
	},
	postExitPlaceholder: {
		id: 'app.settings.default.hooks.post-exit.placeholder',
		defaultMessage: 'Enter post-exit command...',
	},
})

watch(
	settings,
	async () => {
		const setSettings = JSON.parse(JSON.stringify(settings.value))

		setSettings.extra_launch_args = setSettings.launchArgs.trim().split(/\s+/).filter(Boolean)
		setSettings.custom_env_vars = setSettings.envVars
			.trim()
			.split(/\s+/)
			.filter(Boolean)
			.map((x) => x.split('=').filter(Boolean))

		if (!setSettings.hooks.pre_launch) {
			setSettings.hooks.pre_launch = null
		}
		if (!setSettings.hooks.wrapper) {
			setSettings.hooks.wrapper = null
		}
		if (!setSettings.hooks.post_exit) {
			setSettings.hooks.post_exit = null
		}

		if (!setSettings.custom_dir) {
			setSettings.custom_dir = null
		}

		await set(setSettings)
	},
	{ deep: true },
)
</script>

<template>
	<div>
		<h2 class="m-0 text-lg font-extrabold text-contrast">{{ formatMessage(messages.windowSizeTitle) }}</h2>

		<div class="flex items-center justify-between gap-4">
			<div>
				<h3 class="mt-2 m-0 text-base font-extrabold text-primary">
					{{ formatMessage(messages.fullscreenTitle) }}
				</h3>
				<p class="m-0 mt-1 mb-2 leading-tight text-secondary">
					{{ formatMessage(messages.fullscreenDescription) }}
				</p>
			</div>

			<Toggle id="fullscreen" v-model="settings.force_fullscreen" />
		</div>

		<div class="flex items-center justify-between gap-4">
			<div>
				<h3 class="mt-2 m-0 text-base font-extrabold text-primary">{{ formatMessage(messages.widthTitle) }}</h3>
				<p class="m-0 mt-1 mb-2 leading-tight text-secondary">
					{{ formatMessage(messages.widthDescription) }}
				</p>
			</div>

			<input
				id="width"
				v-model="settings.game_resolution[0]"
				:disabled="settings.force_fullscreen"
				autocomplete="off"
				type="number"
				:placeholder="formatMessage(messages.widthPlaceholder)"
			/>
		</div>

		<div class="flex items-center justify-between gap-4">
			<div>
				<h3 class="mt-2 m-0 text-base font-extrabold text-primary">
					{{ formatMessage(messages.heightTitle) }}
				</h3>
				<p class="m-0 mt-1 mb-2 leading-tight text-secondary">
					{{ formatMessage(messages.heightDescription) }}
				</p>
			</div>

			<input
				id="height"
				v-model="settings.game_resolution[1]"
				:disabled="settings.force_fullscreen"
				autocomplete="off"
				type="number"
				class="input"
				:placeholder="formatMessage(messages.heightPlaceholder)"
			/>
		</div>

		<hr class="mt-4 bg-button-border border-none h-[1px]" />

		<h2 class="mt-4 m-0 text-lg font-extrabold text-contrast">{{ formatMessage(messages.memoryTitle) }}</h2>
		<p class="m-0 mt-1 leading-tight">{{ formatMessage(messages.memoryDescription) }}</p>
		<Slider
			id="max-memory"
			v-model="settings.memory.maximum"
			:min="512"
			:max="maxMemory"
			:step="64"
			:snap-points="snapPoints"
			:snap-range="512"
			unit="MB"
		/>

		<h2 class="mt-4 mb-2 text-lg font-extrabold text-contrast">
			{{ formatMessage(messages.javaArgsTitle) }}
		</h2>
		<input
			id="java-args"
			v-model="settings.launchArgs"
			autocomplete="off"
			type="text"
			:placeholder="formatMessage(messages.javaArgsPlaceholder)"
			class="w-full"
		/>

		<h2 class="mt-4 mb-2 text-lg font-extrabold text-contrast">{{ formatMessage(messages.envVarsTitle) }}</h2>
		<input
			id="env-vars"
			v-model="settings.envVars"
			autocomplete="off"
			type="text"
			:placeholder="formatMessage(messages.envVarsPlaceholder)"
			class="w-full"
		/>

		<hr class="mt-4 bg-button-border border-none h-[1px]" />

		<h2 class="mt-4 m-0 text-lg font-extrabold text-contrast">{{ formatMessage(messages.hooksTitle) }}</h2>

		<h3 class="mt-2 m-0 text-base font-extrabold text-primary">{{ formatMessage(messages.preLaunchTitle) }}</h3>
		<p class="m-0 mt-1 mb-2 leading-tight text-secondary">
			{{ formatMessage(messages.preLaunchDescription) }}
		</p>
		<input
			id="pre-launch"
			v-model="settings.hooks.pre_launch"
			autocomplete="off"
			type="text"
			:placeholder="formatMessage(messages.preLaunchPlaceholder)"
			class="w-full"
		/>

		<h3 class="mt-2 m-0 text-base font-extrabold text-primary">{{ formatMessage(messages.wrapperTitle) }}</h3>
		<p class="m-0 mt-1 mb-2 leading-tight text-secondary">{{ formatMessage(messages.wrapperDescription) }}</p>
		<input
			id="wrapper"
			v-model="settings.hooks.wrapper"
			autocomplete="off"
			type="text"
			:placeholder="formatMessage(messages.wrapperPlaceholder)"
			class="w-full"
		/>

		<h3 class="mt-2 m-0 text-base font-extrabold text-primary">{{ formatMessage(messages.postExitTitle) }}</h3>
		<p class="m-0 mt-1 mb-2 leading-tight text-secondary">{{ formatMessage(messages.postExitDescription) }}</p>
		<input
			id="post-exit"
			v-model="settings.hooks.post_exit"
			autocomplete="off"
			type="text"
			:placeholder="formatMessage(messages.postExitPlaceholder)"
			class="w-full"
		/>
	</div>
</template>
