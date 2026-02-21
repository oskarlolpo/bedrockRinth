import { defineStore } from 'pinia'

export const DEFAULT_FEATURE_FLAGS = {
	project_background: false,
	page_path: false,
	worlds_tab: false,
	worlds_in_home: true,
	servers_in_app: false,
}

export const THEME_OPTIONS = ['dark', 'light', 'oled', 'system'] as const
export const ACCENT_OPTIONS = ['green', 'blue', 'purple', 'orange', 'red', 'gray'] as const

export type FeatureFlag = keyof typeof DEFAULT_FEATURE_FLAGS
export type FeatureFlags = Record<FeatureFlag, boolean>
export type ColorTheme = (typeof THEME_OPTIONS)[number]
export type AccentColor = (typeof ACCENT_OPTIONS)[number]

export type ThemeStore = {
	selectedTheme: ColorTheme
	selectedAccent: AccentColor
	advancedRendering: boolean
	toggleSidebar: boolean

	devMode: boolean
	featureFlags: FeatureFlags
}

export const DEFAULT_THEME_STORE: ThemeStore = {
	selectedTheme: 'dark',
	selectedAccent: 'green',
	advancedRendering: true,
	toggleSidebar: false,

	devMode: false,
	featureFlags: DEFAULT_FEATURE_FLAGS,
}

export const useTheming = defineStore('themeStore', {
	state: () => DEFAULT_THEME_STORE,
	actions: {
		setThemeState(newTheme: ColorTheme) {
			if (THEME_OPTIONS.includes(newTheme)) {
				this.selectedTheme = newTheme
			} else {
				console.warn('Selected theme is not present. Check themeOptions.')
			}

			this.setThemeClass()
		},
		setThemeClass() {
			for (const theme of THEME_OPTIONS) {
				document.getElementsByTagName('html')[0].classList.remove(`${theme}-mode`)
			}

			let theme = this.selectedTheme
			if (this.selectedTheme === 'system') {
				const darkThemeMq = window.matchMedia('(prefers-color-scheme: dark)')
				if (darkThemeMq.matches) {
					theme = 'dark'
				} else {
					theme = 'light'
				}
			}

			document.getElementsByTagName('html')[0].classList.add(`${theme}-mode`)
		},
		setAccentState(newAccent: AccentColor) {
			if (ACCENT_OPTIONS.includes(newAccent)) {
				this.selectedAccent = newAccent
				try {
					window.localStorage.setItem('theseus_accent_color', newAccent)
				} catch {
					// ignore local storage failures
				}
			} else {
				console.warn('Selected accent color is not present. Check accent options.')
			}

			this.setAccentVars()
		},
		initializeAccentState() {
			try {
				const stored = window.localStorage.getItem('theseus_accent_color')
				if (stored && ACCENT_OPTIONS.includes(stored as AccentColor)) {
					this.selectedAccent = stored as AccentColor
				}
			} catch {
				// ignore local storage failures
			}

			this.setAccentVars()
		},
		setAccentVars() {
			const root = document.documentElement
			const accent = this.selectedAccent
			root.style.setProperty('--color-brand', `var(--color-${accent})`)
			root.style.setProperty('--color-brand-highlight', `var(--color-${accent}-highlight)`)
			root.style.setProperty('--color-brand-shadow', `var(--color-${accent}-highlight)`)
			root.style.setProperty('--color-button-bg-selected', 'var(--color-brand-highlight)')
			root.style.setProperty('--color-button-text-selected', 'var(--color-brand)')
			root.style.setProperty(
				'--brand-gradient-bg',
				'linear-gradient(0deg, color-mix(in srgb, var(--color-brand) 14%, transparent) 0%, color-mix(in srgb, var(--color-brand) 8%, transparent) 100%)',
			)
			root.style.setProperty(
				'--brand-gradient-strong-bg',
				'linear-gradient(270deg, color-mix(in srgb, var(--color-brand) 18%, var(--color-bg)) 10%, color-mix(in srgb, var(--color-brand) 8%, var(--color-bg)) 100%)',
			)
			root.style.setProperty(
				'--brand-gradient-border',
				'color-mix(in srgb, var(--color-brand) 30%, transparent)',
			)
			root.style.setProperty(
				'--brand-gradient-fade-out-color',
				'linear-gradient(to bottom, color-mix(in srgb, var(--color-brand) 0%, transparent), color-mix(in srgb, var(--color-brand) 16%, var(--color-bg)) 80%)',
			)
		},
		getFeatureFlag(key: FeatureFlag) {
			return this.featureFlags[key] ?? DEFAULT_FEATURE_FLAGS[key]
		},
		getThemeOptions() {
			return THEME_OPTIONS
		},
		getAccentOptions() {
			return ACCENT_OPTIONS
		},
	},
})
