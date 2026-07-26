<script lang="ts">
	import type { AnalysisObservationSnapshot } from 'golden_audio_ui';
	import SoundCardSpectrumCanvas from './SoundCardSpectrumCanvas.svelte';

	let { analysis } = $props<{ analysis: AnalysisObservationSnapshot }>();
</script>

<div class="analysis-grid">
	{#each analysis.taps as tap (tap.tap)}
		{#if tap.result?.kind === 'pitch'}
			<section class="analysis-card">
				<header>
					<h3>Pitch</h3>
					<span>{tap.enabled ? 'Active' : 'Disabled'}</span>
				</header>
				{#if tap.result.value.valid}
					<strong class="pitch">{tap.result.value.note_name}</strong>
					<p>
						{tap.result.value.frequency_hz.toFixed(2)} Hz ·
						{tap.result.value.cents.toFixed(1)} cents
					</p>
					<p>{(tap.result.value.confidence * 100).toFixed(0)}% confidence</p>
				{:else}
					<p>No stable pitch detected.</p>
				{/if}
			</section>
		{:else if tap.result?.kind === 'spectrum'}
			<section class="analysis-card spectrum-card">
				<header>
					<h3>Spectrum</h3>
					<span>{tap.result.value.fft_size} FFT</span>
				</header>
				<SoundCardSpectrumCanvas bands={tap.result.value.bands} label="Spectrum for {tap.source}" />
			</section>
		{/if}
	{/each}

	{#if analysis.taps.length === 0}
		<p class="empty">No analysis observations are available.</p>
	{/if}
</div>

<style>
	.analysis-grid {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(min(16rem, 100%), 1fr));
		gap: 0.7rem;
	}

	.analysis-card {
		padding: 0.65rem;
		border: 0.0625rem solid var(--gc-color-border);
		border-radius: 0.4rem;
		background: var(--gc-color-bg-light);
	}

	.spectrum-card {
		grid-column: span 2;
	}

	header {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: 0.5rem;
		margin-block-end: 0.55rem;
	}

	h3,
	p {
		margin: 0;
	}

	h3 {
		font-size: 0.85rem;
	}

	header span,
	p {
		color: var(--gc-color-text-muted);
		font-size: 0.72rem;
	}

	.pitch {
		display: block;
		margin-block-end: 0.3rem;
		font-size: 1.65rem;
	}

	.empty {
		padding: 0.75rem;
	}

	@media (max-width: 48rem) {
		.spectrum-card {
			grid-column: auto;
		}
	}
</style>
