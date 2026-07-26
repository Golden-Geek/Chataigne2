<script lang="ts">
	import { AudioDeviceSelector, MockAudioDeviceInspectorAdapter } from 'golden_audio_ui';
	import SoundCardAnalysisView from './SoundCardAnalysisView.svelte';
	import SoundCardDiagnostics from './SoundCardDiagnostics.svelte';
	import SoundCardMeters from './SoundCardMeters.svelte';
	import SoundCardPlaybackStatus from './SoundCardPlaybackStatus.svelte';
	import SoundCardRouteMatrix from './SoundCardRouteMatrix.svelte';
	import {
		createMockSoundCardTelemetry,
		mockSoundCardMatrixDestinations,
		mockSoundCardMatrixSources,
		mockSoundCardRoutes
	} from './sound-card-editor-mock';

	const telemetry = createMockSoundCardTelemetry();
	const binding = new MockAudioDeviceInspectorAdapter(telemetry.device);
	const labels = new Map([
		['mock-input-left', 'Input Left'],
		['mock-input-right', 'Input Right'],
		['mock-output-left', 'Output Left'],
		['mock-output-right', 'Output Right']
	]);
</script>

<main class="mock-sound-card-editor" aria-label="Mock Sound Card editor">
	<h1>Sound Card evidence harness</h1>
	<AudioDeviceSelector {binding} />
	<SoundCardMeters inputs={telemetry.inputs} outputs={telemetry.outputs} channelLabels={labels} />
	<SoundCardRouteMatrix
		title="Mock physical input → virtual input"
		rows={mockSoundCardRoutes()}
		sources={mockSoundCardMatrixSources()}
		destinations={mockSoundCardMatrixDestinations()}
		parent={1}
		nodeType="sound_card_input_patch_route"
		sourceDeclId="physical_channel"
		destinationDeclId="virtual_input"
		sourceLabel="Physical input"
		destinationLabel="Virtual input" />
	<SoundCardPlaybackStatus
		moduleNodeId={1}
		activeCount={telemetry.playback.active_voices}
		loadingCount={telemetry.playback.loading_voices}
		voices={telemetry.playback_voices} />
	<SoundCardAnalysisView analysis={telemetry.analysis} />
	<SoundCardDiagnostics {telemetry} xruns={telemetry.runtime.xrun_count} lastError={null} />
</main>

<style>
	.mock-sound-card-editor {
		display: grid;
		gap: 0.8rem;
		max-inline-size: 64rem;
		padding: 1rem;
		background: var(--gc-color-bg, #111722);
		color: var(--gc-color-text, #f1f4f8);
	}

	h1 {
		margin: 0;
		font-size: 1.1rem;
	}
</style>
