import IntlMessageFormat from 'intl-messageformat'
import type { Ref } from 'vue'
import type { CompileError, MessageCompiler, MessageContext } from 'vue-i18n'

import { injectI18n } from '../providers/i18n'

export interface MessageDescriptor {
	id: string
	defaultMessage?: string
	description?: string
}

export type MessageDescriptorMap<K extends string> = Record<K, MessageDescriptor>

export type CrowdinMessages = Record<string, { message: string } | string>

export function defineMessage<T extends MessageDescriptor>(descriptor: T): T {
	return descriptor
}

export function defineMessages<K extends string, T extends MessageDescriptorMap<K>>(
	descriptors: T,
): T {
	return descriptors
}

export interface LocaleDefinition {
	code: string
	name: string
	dir?: 'ltr' | 'rtl'
	iso?: string
	file?: string
}

export const LOCALES: LocaleDefinition[] = [
	// { code: 'af-ZA', name: 'Afrikaans' },
	// { code: 'ar-EG', name: 'Ш§Щ„Ш№Ш±ШЁЩЉШ© (Щ…ШµШ±)', dir: 'rtl' },
	// { code: 'ar-SA', name: 'Ш§Щ„Ш№Ш±ШЁЩЉШ© (Ш§Щ„ШіШ№Щ€ШЇЩЉШ©)', dir: 'rtl' },
	// { code: 'az-AZ', name: 'AzЙ™rbaycan' },
	// { code: 'be-BY', name: 'Р‘РµР»Р°СЂСѓСЃРєР°СЏ' },
	// { code: 'bg-BG', name: 'Р‘СЉР»РіР°СЂСЃРєРё' },
	// { code: 'bn-BD', name: 'а¦¬а¦ѕа¦‚а¦Іа¦ѕ' },
	// { code: 'ca-ES', name: 'CatalГ ' },
	// { code: 'ceb-PH', name: 'Cebuano' },
	// { code: 'cs-CZ', name: 'ДЊeЕЎtina' },
	// { code: 'da-DK', name: 'Dansk' },
	{ code: 'de-CH', name: 'Deutsch (Schweiz)' },
	{ code: 'de-DE', name: 'Deutsch' },
	// { code: 'el-GR', name: 'О•О»О»О·ОЅО№ОєО¬' },
	// { code: 'en-PT', name: 'Pirate English' },
	// { code: 'en-UD', name: 'Upside Down' },
	{ code: 'en-US', name: 'English (United States)' },
	// { code: 'eo-UY', name: 'Esperanto' },
	{ code: 'es-419', name: 'Español (Latinoamérica)' },
	{ code: 'es-ES', name: 'Español (España)' },
	// { code: 'et-EE', name: 'Eesti' },
	// { code: 'fa-IR', name: 'ЩЃШ§Ш±ШіЫЊ', dir: 'rtl' },
	// { code: 'fi-FI', name: 'Suomi' },
	// { code: 'fil-PH', name: 'Filipino' },
	{ code: 'fr-FR', name: 'Français' },
	// { code: 'he-IL', name: 'ЧўЧ‘ЧЁЧ™ЧЄ', dir: 'rtl' },
	// { code: 'hi-IN', name: 'а¤№а¤їа¤ЁаҐЌа¤¦аҐЂ' },
	// { code: 'hr-HR', name: 'Hrvatski' },
	// { code: 'hu-HU', name: 'Magyar' },
	// { code: 'id-ID', name: 'Bahasa Indonesia' },
	// { code: 'is-IS', name: 'ГЌslenska' },
	{ code: 'it-IT', name: 'Italiano' },
	// { code: 'ja-JP', name: 'ж—Ґжњ¬иЄћ' },
	// { code: 'kk-KZ', name: 'ТљР°Р·Р°Т›С€Р°' },
	// { code: 'ko-KR', name: 'н•њкµ­м–ґ' },
	// { code: 'ky-KG', name: 'РљС‹СЂРіС‹Р·С‡Р°' },
	// { code: 'lol-US', name: 'LOLCAT' },
	// { code: 'lt-LT', name: 'LietuviЕі' },
	// { code: 'lv-LV', name: 'LatvieЕЎu' },
	// { code: 'ms-Arab', name: 'ШЁЩ‡Ш§Ші Щ…Щ„Ш§ЩЉЩ€ (Ш¬Ш§Щ€ЩЉ)', dir: 'rtl' },
	{ code: 'ms-MY', name: 'Bahasa Melayu' },
	// { code: 'nl-NL', name: 'Nederlands' },
	// { code: 'no-NO', name: 'Norsk' },
	{ code: 'pl-PL', name: 'Polski' },
	{ code: 'pt-BR', name: 'Português (Brasil)' },
	{ code: 'pt-PT', name: 'Português (Portugal)' },
	// { code: 'ro-RO', name: 'Română' },
	{ code: 'ru-RU', name: 'Русский' },
	{ code: 'ru-XM', name: 'Русский (матершинный)' },
	// { code: 'sk-SK', name: 'Slovenčina' },
	// { code: 'sl-SI', name: 'SlovenЕЎДЌina' },
	// { code: 'sr-CS', name: 'РЎСЂРїСЃРєРё (С›РёСЂРёР»РёС†Р°)' },
	// { code: 'sr-SP', name: 'Srpski (latinica)' },
	// { code: 'sv-SE', name: 'Svenska' },
	// { code: 'th-TH', name: 'а№„аё—аёў' },
	// { code: 'tl-PH', name: 'Tagalog' },
	{ code: 'tr-TR', name: 'Türkçe' },
	// { code: 'tt-RU', name: 'РўР°С‚Р°СЂС‡Р°' },
	{ code: 'uk-UA', name: 'Українська' },
	// { code: 'vi-VN', name: 'Tiбєїng Viб»‡t' },
	{ code: 'zh-CN', name: '简体中文' },
	{ code: 'zh-TW', name: '繁體中文' },
]

export function transformCrowdinMessages(messages: CrowdinMessages): Record<string, string> {
	const result: Record<string, string> = {}
	for (const [key, value] of Object.entries(messages)) {
		if (typeof value === 'string') {
			result[key] = value
		} else if (typeof value === 'object' && value !== null && 'message' in value) {
			result[key] = value.message
		}
	}
	return result
}

const LOCALE_CODES = new Set(LOCALES.map((l) => l.code))

/**
 * Builds locale messages from glob-imported modules.
 * Only includes locales that are defined in the LOCALES array.
 * Usage: buildLocaleMessages(import.meta.glob('./locales/* /index.json', { eager: true }))
 */
export function buildLocaleMessages(
	modules: Record<string, { default: CrowdinMessages }>,
): Record<string, Record<string, string>> {
	const messages: Record<string, Record<string, string>> = {}
	for (const [path, module] of Object.entries(modules)) {
		// Extract locale code from path like './locales/en-US/index.json' or './src/locales/en-US/index.json'
		const match = path.match(/\/([^/]+)\/index\.json$/)
		if (match) {
			const locale = match[1]
			// Only include locales that are in our LOCALES list
			if (LOCALE_CODES.has(locale)) {
				messages[locale] = transformCrowdinMessages(module.default)
			}
		}
	}
	return messages
}

/**
 * Creates a vue-i18n message compiler that uses IntlMessageFormat for ICU syntax support.
 * This enables pluralization, select, and other ICU message features.
 */
export function createMessageCompiler(): MessageCompiler {
	return (msg, { locale, key, onError }) => {
		let messageString: string

		if (typeof msg === 'string') {
			messageString = msg
		} else if (typeof msg === 'object' && msg !== null && 'message' in msg) {
			messageString = (msg as { message: string }).message
		} else {
			onError?.(new Error('Invalid message format') as CompileError)
			return () => key
		}

		try {
			const formatter = new IntlMessageFormat(messageString, locale)
			return (ctx: MessageContext) => {
				try {
					return formatter.format(ctx.values as Record<string, unknown>) as string
				} catch {
					return messageString
				}
			}
		} catch (e) {
			onError?.(e as CompileError)
			return () => key
		}
	}
}
export interface VIntlFormatters {
	formatMessage(descriptor: MessageDescriptor, values?: Record<string, unknown>): string
}

/**
 * Composable that provides formatMessage() with the same API as @vintl/vintl.
 * Uses the injected I18nContext from the provider.
 */
export function useVIntl(): VIntlFormatters & { locale: Ref<string> } {
	const { t, locale } = injectI18n()

	function formatMessage(descriptor: MessageDescriptor, values?: Record<string, unknown>): string {
		const key = descriptor.id
		const translation = t(key, values ?? {})

		if (translation && translation !== key) {
			return translation as string
		}

		// Fallback to defaultMessage if key not found
		const defaultMsg = descriptor.defaultMessage ?? key
		try {
			const formatter = new IntlMessageFormat(defaultMsg, locale.value)
			return formatter.format(values ?? {}) as string
		} catch {
			return defaultMsg
		}
	}

	return { formatMessage, locale }
}

