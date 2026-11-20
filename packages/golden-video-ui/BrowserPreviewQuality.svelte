<script lang="ts">
	import { useBrowserPreviews, type BrowserPreviewMode } from './browser-preview.svelte';

	const previews = useBrowserPreviews();
	const choices: { value: BrowserPreviewMode; label: string }[] = [
		{ value: 'disabled', label: 'Disabled' },
		{ value: 'auto', label: 'Auto' },
		{ value: 'low', label: 'Forced Low Quality' },
		{ value: 'high', label: 'Forced High Quality' }
	];
</script>

<select
	aria-label="Browser Preview Quality"
	title={previews.mode === 'auto'
		? `Browser Preview Quality · Auto (${previews.quality})`
		: 'Browser Preview Quality'}
	data-effective-quality={previews.quality ?? 'disabled'}
	bind:value={() => previews.mode, (mode) => previews.setMode(mode)}
>
	{#each choices as choice (choice.value)}
		<option value={choice.value}>{choice.label}</option>
	{/each}
</select>

<style>
	select {
		margin: 0 0.25rem;
		padding: 0.16em 0.4em;
		border: 1px solid var(--gc-border-color);
		border-radius: 0.3em;
		background: var(--gc-input-background);
		color: inherit;
		font: inherit;
	}
</style>
