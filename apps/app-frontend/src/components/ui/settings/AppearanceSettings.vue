<script setup lang="ts">
import { Combobox, defineMessages, ThemeSelector, Toggle, useVIntl } from '@modrinth/ui'
import { ref, watch } from 'vue'

import { get, set } from '@/helpers/settings.ts'
import { getOS } from '@/helpers/utils'
import { useTheming } from '@/store/state'
import type { AccentColor, ColorTheme } from '@/store/theme.ts'

const themeStore = useTheming()
const { formatMessage } = useVIntl()

const os = ref(await getOS())
const settings = ref(await get())
const accentOptions: { value: AccentColor; label: string }[] = [
	{ value: 'green', label: 'Green' },
	{ value: 'blue', label: 'Blue' },
	{ value: 'purple', label: 'Purple' },
	{ value: 'orange', label: 'Orange' },
	{ value: 'red', label: 'Red' },
	{ value: 'gray', label: 'Gray' },
]

const messages = defineMessages({
	colorThemeTitle: {
		id: 'app.settings.appearance.color-theme.title',
		defaultMessage: 'Color theme',
	},
	colorThemeDescription: {
		id: 'app.settings.appearance.color-theme.description',
		defaultMessage: 'Select your preferred color theme for Modrinth App.',
	},
	accentColorTitle: {
		id: 'app.settings.appearance.accent-color.title',
		defaultMessage: 'Accent color',
	},
	accentColorDescription: {
		id: 'app.settings.appearance.accent-color.description',
		defaultMessage: 'Choose the primary accent color used across the launcher UI.',
	},
	accentColorDropdown: {
		id: 'app.settings.appearance.accent-color.dropdown',
		defaultMessage: 'Accent color dropdown',
	},
	advancedRenderingTitle: {
		id: 'app.settings.appearance.advanced-rendering.title',
		defaultMessage: 'Advanced rendering',
	},
	advancedRenderingDescription: {
		id: 'app.settings.appearance.advanced-rendering.description',
		defaultMessage:
			'Enables advanced rendering such as blur effects that may cause performance issues without hardware-accelerated rendering.',
	},
	hideNametagTitle: {
		id: 'app.settings.appearance.hide-nametag.title',
		defaultMessage: 'Hide nametag',
	},
	hideNametagDescription: {
		id: 'app.settings.appearance.hide-nametag.description',
		defaultMessage: 'Disables the nametag above your player on the skins page.',
	},
	nativeDecorationsTitle: {
		id: 'app.settings.appearance.native-decorations.title',
		defaultMessage: 'Native decorations',
	},
	nativeDecorationsDescription: {
		id: 'app.settings.appearance.native-decorations.description',
		defaultMessage: 'Use system window frame (app restart required).',
	},
	minimizeLauncherTitle: {
		id: 'app.settings.appearance.minimize-launcher.title',
		defaultMessage: 'Minimize launcher',
	},
	minimizeLauncherDescription: {
		id: 'app.settings.appearance.minimize-launcher.description',
		defaultMessage: 'Minimize the launcher when a Minecraft process starts.',
	},
	defaultLandingPageTitle: {
		id: 'app.settings.appearance.default-landing-page.title',
		defaultMessage: 'Default landing page',
	},
	defaultLandingPageDescription: {
		id: 'app.settings.appearance.default-landing-page.description',
		defaultMessage: 'Change the page to which the launcher opens on.',
	},
	openingPageDropdown: {
		id: 'app.settings.appearance.default-landing-page.dropdown',
		defaultMessage: 'Opening page dropdown',
	},
	selectOption: {
		id: 'app.settings.appearance.select-option',
		defaultMessage: 'Select an option',
	},
	jumpBackIntoWorldsTitle: {
		id: 'app.settings.appearance.jump-back-into-worlds.title',
		defaultMessage: 'Jump back into worlds',
	},
	jumpBackIntoWorldsDescription: {
		id: 'app.settings.appearance.jump-back-into-worlds.description',
		defaultMessage: 'Includes recent worlds in the "Jump back in" section on the Home page.',
	},
	toggleSidebarTitle: {
		id: 'app.settings.appearance.toggle-sidebar.title',
		defaultMessage: 'Toggle sidebar',
	},
	toggleSidebarDescription: {
		id: 'app.settings.appearance.toggle-sidebar.description',
		defaultMessage: 'Enables the ability to toggle the sidebar.',
	},
	pageHome: {
		id: 'app.settings.appearance.page.home',
		defaultMessage: 'Home',
	},
	pageLibrary: {
		id: 'app.settings.appearance.page.library',
		defaultMessage: 'Library',
	},
})

watch(
	settings,
	async () => {
		await set(settings.value)
	},
	{ deep: true },
)
</script>
<template>
	<h2 class="m-0 text-lg font-extrabold text-contrast">{{ formatMessage(messages.colorThemeTitle) }}</h2>
	<p class="m-0 mt-1">{{ formatMessage(messages.colorThemeDescription) }}</p>

	<ThemeSelector
		:update-color-theme="
			(theme: ColorTheme) => {
				themeStore.setThemeState(theme)
				settings.theme = theme
			}
		"
		:current-theme="settings.theme"
		:theme-options="themeStore.getThemeOptions()"
		system-theme-color="system"
	/>

	<div class="mt-4 flex items-center justify-between gap-4">
		<div>
			<h2 class="m-0 text-lg font-extrabold text-contrast">{{ formatMessage(messages.accentColorTitle) }}</h2>
			<p class="m-0 mt-1">{{ formatMessage(messages.accentColorDescription) }}</p>
		</div>
		<Combobox
			id="accent-color"
			:model-value="themeStore.selectedAccent"
			:name="formatMessage(messages.accentColorDropdown)"
			class="w-40"
			:options="accentOptions"
			:display-value="
				accentOptions.find((option) => option.value === themeStore.selectedAccent)?.label ??
				themeStore.selectedAccent
			"
			@update:model-value="
				(value) => {
					themeStore.setAccentState(value as AccentColor)
				}
			"
		/>
	</div>

	<div class="mt-4 flex items-center justify-between">
		<div>
			<h2 class="m-0 text-lg font-extrabold text-contrast">
				{{ formatMessage(messages.advancedRenderingTitle) }}
			</h2>
			<p class="m-0 mt-1">{{ formatMessage(messages.advancedRenderingDescription) }}</p>
		</div>

		<Toggle
			id="advanced-rendering"
			:model-value="themeStore.advancedRendering"
			@update:model-value="
				(e) => {
					themeStore.advancedRendering = !!e
					settings.advanced_rendering = themeStore.advancedRendering
				}
			"
		/>
	</div>

	<div class="mt-4 flex items-center justify-between">
		<div>
			<h2 class="m-0 text-lg font-extrabold text-contrast">{{ formatMessage(messages.hideNametagTitle) }}</h2>
			<p class="m-0 mt-1">{{ formatMessage(messages.hideNametagDescription) }}</p>
		</div>
		<Toggle id="hide-nametag-skins-page" v-model="settings.hide_nametag_skins_page" />
	</div>

	<div v-if="os !== 'MacOS'" class="mt-4 flex items-center justify-between gap-4">
		<div>
			<h2 class="m-0 text-lg font-extrabold text-contrast">
				{{ formatMessage(messages.nativeDecorationsTitle) }}
			</h2>
			<p class="m-0 mt-1">{{ formatMessage(messages.nativeDecorationsDescription) }}</p>
		</div>
		<Toggle id="native-decorations" v-model="settings.native_decorations" />
	</div>

	<div class="mt-4 flex items-center justify-between">
		<div>
			<h2 class="m-0 text-lg font-extrabold text-contrast">
				{{ formatMessage(messages.minimizeLauncherTitle) }}
			</h2>
			<p class="m-0 mt-1">{{ formatMessage(messages.minimizeLauncherDescription) }}</p>
		</div>
		<Toggle id="minimize-launcher" v-model="settings.hide_on_process_start" />
	</div>

	<div class="mt-4 flex items-center justify-between">
		<div>
			<h2 class="m-0 text-lg font-extrabold text-contrast">
				{{ formatMessage(messages.defaultLandingPageTitle) }}
			</h2>
			<p class="m-0 mt-1">{{ formatMessage(messages.defaultLandingPageDescription) }}</p>
		</div>
		<Combobox
			id="opening-page"
			v-model="settings.default_page"
			:name="formatMessage(messages.openingPageDropdown)"
			class="w-40"
			:options="[
				{ value: 'Home', label: formatMessage(messages.pageHome) },
				{ value: 'Library', label: formatMessage(messages.pageLibrary) },
			]"
			:display-value="
				settings.default_page === 'Library'
					? formatMessage(messages.pageLibrary)
					: settings.default_page === 'Home'
						? formatMessage(messages.pageHome)
						: formatMessage(messages.selectOption)
			"
		/>
	</div>

	<div class="mt-4 flex items-center justify-between">
		<div>
			<h2 class="m-0 text-lg font-extrabold text-contrast">
				{{ formatMessage(messages.jumpBackIntoWorldsTitle) }}
			</h2>
			<p class="m-0 mt-1">{{ formatMessage(messages.jumpBackIntoWorldsDescription) }}</p>
		</div>
		<Toggle
			:model-value="themeStore.getFeatureFlag('worlds_in_home')"
			@update:model-value="
				() => {
					const newValue = !themeStore.getFeatureFlag('worlds_in_home')
					themeStore.featureFlags['worlds_in_home'] = newValue
					settings.feature_flags['worlds_in_home'] = newValue
				}
			"
		/>
	</div>

	<div class="mt-4 flex items-center justify-between">
		<div>
			<h2 class="m-0 text-lg font-extrabold text-contrast">{{ formatMessage(messages.toggleSidebarTitle) }}</h2>
			<p class="m-0 mt-1">{{ formatMessage(messages.toggleSidebarDescription) }}</p>
		</div>
		<Toggle
			id="toggle-sidebar"
			:model-value="settings.toggle_sidebar"
			@update:model-value="
				(e) => {
					settings.toggle_sidebar = !!e
					themeStore.toggleSidebar = settings.toggle_sidebar
				}
			"
		/>
	</div>
</template>
