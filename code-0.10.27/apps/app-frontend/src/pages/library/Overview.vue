<script setup>
import { defineMessages, useVIntl } from '@modrinth/ui'
import GridDisplay from '@/components/GridDisplay.vue'

const { formatMessage } = useVIntl()
const messages = defineMessages({
	instances: {
		id: 'library.instances',
		defaultMessage: 'Instances',
	},
})

defineProps({
	instances: {
		type: Array,
		required: true,
	},
})
</script>
<template>
	<GridDisplay
		v-if="instances.length > 0"
		:label="formatMessage(messages.instances)"
		:instances="instances"
	/>
</template>
