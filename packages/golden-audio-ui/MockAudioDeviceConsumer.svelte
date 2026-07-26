<script lang="ts">
	import AudioDeviceSelector from './AudioDeviceSelector.svelte';
	import { MockAudioDeviceInspectorAdapter } from './mock.svelte';

	let { adapter = new MockAudioDeviceInspectorAdapter() } = $props<{
		adapter?: MockAudioDeviceInspectorAdapter;
	}>();
</script>

<section class="mock-audio-consumer" aria-label="Golden Audio mock consumer">
	<AudioDeviceSelector binding={adapter} />
</section>

<style>
	.mock-audio-consumer {
		max-inline-size: 48rem;
		padding: 1rem;
		background: #151922;
		color: #f3f6fb;
	}
</style>
