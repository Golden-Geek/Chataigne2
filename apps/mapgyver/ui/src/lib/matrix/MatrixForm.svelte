<script lang="ts">
	import type { MatrixDefinition } from '../media/generated/MatrixDefinition';
	import type { MatrixEdit } from '../media/generated/MatrixEdit';
	import type { TransitionCurve } from '../media/generated/TransitionCurve';
	import TransitionFields from './TransitionFields.svelte';
	let {
		definition,
		disabled,
		edit
	}: { definition: MatrixDefinition; disabled: boolean; edit(value: MatrixEdit): unknown } =
		$props();
</script>

<form
	class="matrix-authoring"
	onsubmit={(event) => {
		event.preventDefault();
		const data = new FormData(event.currentTarget);
		edit({
			kind: 'Configure',
			definition: {
				...definition,
				canvas: { width: Number(data.get('width')), height: Number(data.get('height')) },
				transition: {
					duration_ms: Number(data.get('duration')),
					curve: String(data.get('curve')) as TransitionCurve
				}
			}
		});
	}}>
	<fieldset {disabled}>
		<legend>Matrix settings</legend>
		<label
			>Canvas width<input
				name="width"
				type="number"
				min="1"
				max="16384"
				step="1"
				required
				value={definition.canvas.width} /></label>
		<label
			>Canvas height<input
				name="height"
				type="number"
				min="1"
				max="16384"
				step="1"
				required
				value={definition.canvas.height} /></label>
		<label
			>Empty cells<select disabled value={definition.empty_cell}
				><option value={definition.empty_cell}>{definition.empty_cell}</option></select
			></label>
		<TransitionFields value={definition.transition} />
		<button>Save Matrix settings</button>
	</fieldset>
</form>
