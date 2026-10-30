<script lang="ts">
	import type { ScreenSnapshot } from './generated/ScreenSnapshot';
	import type { ScreenDefinition } from './generated/ScreenDefinition';
	import type { ScreenTarget } from './generated/ScreenTarget';

	let {
		screen,
		disabled,
		configure
	}: {
		screen: ScreenSnapshot;
		disabled: boolean;
		configure: (definition: ScreenDefinition) => Promise<unknown>;
	} = $props();
	let mode = $state<ScreenTarget['kind'] | 'None'>('None');
	let enabled = $state(false);
	let display = $state('');
	let sender = $state('');
	let left = $state<number>();
	let top = $state<number>();
	let width = $state<number>();
	let height = $state<number>();
	const authored = $derived(JSON.stringify(screen.definition));
	$effect(() => {
		const definition: ScreenDefinition = JSON.parse(authored);
		const target = definition.target;
		mode = target?.kind ?? 'None';
		enabled = definition.enabled;
		display = target?.kind === 'Fullscreen' ? target.display : '';
		sender = target?.kind === 'Spout' ? target.sender : '';
		left = target?.kind === 'Rectangle' ? target.bounds.left : undefined;
		top = target?.kind === 'Rectangle' ? target.bounds.top : undefined;
		const extent =
			target?.kind === 'Rectangle'
				? target.bounds.extent
				: target?.kind === 'Spout'
					? target.extent
					: undefined;
		width = extent?.width;
		height = extent?.height;
	});
	async function submit(event: SubmitEvent) {
		event.preventDefault();
		let target: ScreenTarget | null = null;
		if (mode === 'Fullscreen') target = { kind: mode, display };
		else if (mode === 'Spout') {
			if (width === undefined || height === undefined) return;
			target = { kind: mode, sender, extent: { width, height } };
		} else if (mode === 'Rectangle') {
			if (left === undefined || top === undefined || width === undefined || height === undefined)
				return;
			target = { kind: mode, bounds: { left, top, extent: { width, height } } };
		}
		await configure({ target, enabled });
	}
</script>

<form onsubmit={submit}>
	<fieldset {disabled}>
		<legend>Screen target · {screen.label}</legend>
		<label class="toggle"><input type="checkbox" bind:checked={enabled} /> Enabled</label>
		<label
			>Target<select bind:value={mode}>
				<option value="None">Unconfigured</option>
				<option value="Fullscreen">Fullscreen display</option>
				<option value="Rectangle">Desktop rectangle</option>
				<option value="Spout">Virtual Spout output</option>
			</select></label>
		{#if mode === 'Fullscreen'}
			<label
				>Display identity<input
					bind:value={display}
					required
					placeholder="Stable display identifier" /></label>
		{:else if mode === 'Spout'}
			<label>Sender name<input bind:value={sender} required /></label>
		{/if}
		{#if mode === 'Rectangle'}
			<div class="fields">
				<label>Left (physical px)<input type="number" step="1" bind:value={left} required /></label>
				<label>Top (physical px)<input type="number" step="1" bind:value={top} required /></label>
			</div>
		{/if}
		{#if mode === 'Rectangle' || mode === 'Spout'}
			<div class="fields">
				<label
					>Width ({mode === 'Rectangle' ? 'physical px' : 'texels'})<input
						type="number"
						min="1"
						step="1"
						bind:value={width}
						required /></label>
				<label
					>Height ({mode === 'Rectangle' ? 'physical px' : 'texels'})<input
						type="number"
						min="1"
						step="1"
						bind:value={height}
						required /></label>
			</div>
		{/if}
		<button type="submit">Apply screen target</button>
	</fieldset>
</form>
