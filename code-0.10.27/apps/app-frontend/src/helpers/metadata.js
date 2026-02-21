import { invoke } from '@tauri-apps/api/core'

/// Gets the game versions from daedalus
// Returns a VersionManifest
export async function get_game_versions() {
	return await invoke('plugin:metadata|metadata_get_game_versions')
}

// Gets the given loader versions from daedalus
// Returns Manifest
export async function get_loader_versions(loader) {
	return await invoke('plugin:metadata|metadata_get_loader_versions', { loader })
}

// Gets Bedrock versions as tuples: [version, packageId, 0]
export async function get_bedrock_versions() {
	return await invoke('plugin:metadata|metadata_get_bedrock_versions')
}

// Gets Bedrock content list from mcpehub
// kind: 'textures' | 'shaders' | 'maps'
export async function get_bedrock_content(kind, page = 1) {
	return await invoke('plugin:metadata|metadata_get_bedrock_content', {
		kind,
		page,
	})
}

export async function get_bedrock_content_details(kind, pageUrl) {
	return await invoke('plugin:metadata|metadata_get_bedrock_content_details', {
		kind,
		pageUrl,
	})
}

// Downloads and installs a Bedrock content item into profile files
export async function install_bedrock_content(profilePath, kind, pageUrl) {
	return await invoke('plugin:metadata|metadata_install_bedrock_content', {
		profilePath,
		kind,
		pageUrl,
	})
}

export async function get_installed_bedrock_content(profilePath) {
	return await invoke('plugin:metadata|metadata_get_installed_bedrock_content', {
		profilePath,
	})
}
