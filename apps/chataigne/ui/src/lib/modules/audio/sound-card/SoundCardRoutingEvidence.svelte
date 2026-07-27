<script lang="ts">
	import {
		AudioRoutingPatchBay,
		type AudioRoutingPatchBinding,
		type AudioRoutingPatchConnection,
		type AudioRoutingPatchEndpoint
	} from 'golden_audio_ui';

	const sources: readonly AudioRoutingPatchEndpoint[] = Array.from({ length: 4 }, (_, index) => ({
		id: `output-${index + 1}`,
		label: `Output ${index + 1}`,
		editable: true
	}));
	const destinations: readonly AudioRoutingPatchEndpoint[] = Array.from(
		{ length: 6 },
		(_, index) => ({
			id: `input-${index + 1}`,
			label: `Input ${index + 1}`
		})
	);
	const connections: readonly AudioRoutingPatchConnection[] = [
		{ id: 'route-1', sourceId: 'output-1', destinationId: 'input-1' },
		{ id: 'route-2', sourceId: 'output-2', destinationId: 'input-2' },
		{ id: 'route-3', sourceId: 'output-2', destinationId: 'input-5' },
		{ id: 'route-4', sourceId: 'output-3', destinationId: 'input-6' },
		{ id: 'route-5', sourceId: 'output-4', destinationId: 'input-3' }
	];
	const binding: AudioRoutingPatchBinding = {
		connect: async () => true,
		disconnect: async () => true,
		renameEndpoint: async () => true
	};
</script>

<main aria-label="Sound Card routing evidence">
	<h1>Output Routing</h1>
	<AudioRoutingPatchBay
		{sources}
		{destinations}
		{connections}
		{binding}
		sourceLabel="Output Channels"
		destinationLabel="Device Outputs" />
</main>

<style>
	main {
		display: grid;
		gap: 0.8rem;
		max-inline-size: 42rem;
		padding: 1rem;
		background: var(--gc-color-bg, #202020);
		color: var(--gc-color-text, #d5d5d5);
	}

	h1 {
		margin: 0;
		font-size: 1.1rem;
	}
</style>
