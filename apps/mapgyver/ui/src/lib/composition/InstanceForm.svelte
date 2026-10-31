<script lang="ts">
	import type { InstanceSnapshot } from '../media/generated/InstanceSnapshot';
	import type { InstanceDefinition } from '../media/generated/InstanceDefinition';
	import type { BlendMode } from '../media/generated/BlendMode';
	import type { MediaManagerSnapshot } from '../media/generated/MediaManagerSnapshot';
	import { channelReferenceFromKey } from '../media/identity';
	import SourcePicker from './SourcePicker.svelte';
	let {
		instance,
		catalog,
		disabled,
		configure,
		rename
	}: {
		instance: InstanceSnapshot;
		catalog: MediaManagerSnapshot;
		disabled: boolean;
		configure(definition: InstanceDefinition): unknown;
		rename(label: string): unknown;
	} = $props();
	const definition = $derived(instance.definition);
	const blends: BlendMode[] = ['Normal', 'Add', 'Multiply', 'Screen'];

	function submit(event: SubmitEvent & { currentTarget: HTMLFormElement }) {
		event.preventDefault();
		const values = new FormData(event.currentTarget);
		const key = String(values.get('source'));
		const source = key ? channelReferenceFromKey(key) : null;
		const number = (name: string) => Number(values.get(name));
		configure({
			...definition,
			source,
			transform: {
				position: [number('positionX'), number('positionY')],
				scale: [number('scaleX'), number('scaleY')],
				rotation: number('rotation'),
				pivot: [number('pivotX'), number('pivotY')]
			},
			opacity: number('opacity'),
			blend: String(values.get('blend')) as BlendMode,
			enabled: values.has('enabled')
		});
	}
</script>

<section aria-label="Selected instance">
	<form
		onsubmit={(event) => {
			event.preventDefault();
			rename(String(new FormData(event.currentTarget).get('label')));
		}}>
		<fieldset {disabled}>
			<legend>Instance name</legend>
			<input name="label" value={instance.label} required aria-label="Instance name" />
			<button>Rename</button>
		</fieldset>
	</form>
	<form onsubmit={submit}>
		<fieldset {disabled}>
			<legend>Source and transform</legend>
			<div class="source">
				<span>Source</span><SourcePicker {catalog} selected={definition.source} />
			</div>
			<div class="pairs">
				<label
					>Position X<input
						name="positionX"
						type="number"
						step="any"
						value={definition.transform.position[0]}
						required /></label>
				<label
					>Position Y<input
						name="positionY"
						type="number"
						step="any"
						value={definition.transform.position[1]}
						required /></label>
				<label
					>Scale X<input
						name="scaleX"
						type="number"
						step="any"
						value={definition.transform.scale[0]}
						required /></label>
				<label
					>Scale Y<input
						name="scaleY"
						type="number"
						step="any"
						value={definition.transform.scale[1]}
						required /></label>
				<label
					>Pivot X<input
						name="pivotX"
						type="number"
						step="any"
						value={definition.transform.pivot[0]}
						required /></label>
				<label
					>Pivot Y<input
						name="pivotY"
						type="number"
						step="any"
						value={definition.transform.pivot[1]}
						required /></label>
			</div>
			<label
				>Rotation (radians, clockwise)<input
					name="rotation"
					type="number"
					step="any"
					value={definition.transform.rotation}
					required /></label>
			<label
				>Opacity<input
					name="opacity"
					type="number"
					step="any"
					min="0"
					max="1"
					value={definition.opacity}
					required /></label>
			<label
				>Blend<select name="blend" value={definition.blend}>
					{#each blends as blend}<option value={blend}>{blend}</option>{/each}
				</select></label>
			<label class="enabled"
				><input name="enabled" type="checkbox" checked={definition.enabled} />Enabled</label>
			<p>
				Position uses canvas texels; pivot uses source fractions. Negative scale mirrors the source.
			</p>
			<button>Apply instance</button>
		</fieldset>
	</form>
</section>

<style>
	.source,
	section,
	fieldset,
	label {
		display: grid;
		gap: 0.5rem;
	}
	section {
		gap: 1rem;
	}
	fieldset {
		border: 0.0625rem solid #444b57;
		border-radius: 0.4rem;
		padding: 0.75rem;
		min-width: 0;
	}
	.pairs {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 0.5rem;
	}
	input,
	select {
		min-width: 0;
		width: 100%;
	}
	.enabled {
		display: flex;
		align-items: center;
	}
	.enabled input {
		width: auto;
	}
	p {
		font-size: 0.85em;
		opacity: 0.8;
		margin: 0.25em 0;
	}
</style>
