<script lang="ts">
	import { untrack } from 'svelte';
	import type { WorkbenchSession } from 'golden_ui';
	import type { ColumnSnapshot } from '../media/generated/ColumnSnapshot';
	import type { MatrixEdit } from '../media/generated/MatrixEdit';
	import type { MatrixTargetId } from '../media/generated/MatrixTargetId';
	import type { TransitionDefinition } from '../media/generated/TransitionDefinition';
	import type { TransitionCurve } from '../media/generated/TransitionCurve';
	import NameForm from './NameForm.svelte';
	import TransitionFields from './TransitionFields.svelte';
	import TargetPicker from './TargetPicker.svelte';
	let {
		column,
		transition,
		media,
		session,
		disabled,
		edit
	}: {
		column: ColumnSnapshot;
		transition: TransitionDefinition;
		media: string;
		session: WorkbenchSession | null;
		disabled: boolean;
		edit(value: MatrixEdit): unknown;
	} = $props();
	let override = $state(untrack(() => column.definition.transition !== null));
	let after = $state<MatrixTargetId | null>(null);
</script>

<NameForm
	label="Column name"
	value={column.label}
	{disabled}
	rename={(label) => edit({ kind: 'RenameColumn', column: column.id, label })} />
<form
	class="matrix-authoring"
	onsubmit={(event) => {
		event.preventDefault();
		const data = new FormData(event.currentTarget);
		edit({
			kind: 'ConfigureColumn',
			column: column.id,
			definition: {
				transition: override
					? {
							duration_ms: Number(data.get('duration')),
							curve: String(data.get('curve')) as TransitionCurve
						}
					: null
			}
		});
	}}>
	<fieldset {disabled}>
		<legend>Column transition</legend>
		<label><input type="checkbox" bind:checked={override} />Override Matrix transition</label>
		{#if override}<TransitionFields value={column.definition.transition ?? transition} />
		{:else}<p>Uses the Matrix transition: {transition.duration_ms} ms / {transition.curve}.</p>{/if}
		<button>Save column transition</button>
	</fieldset>
</form>
<form
	class="matrix-authoring"
	onsubmit={(event) => {
		event.preventDefault();
		edit({
			kind: 'ReorderColumn',
			column: column.id,
			after: after?.kind === 'Column' ? after.column : null
		});
	}}>
	<fieldset {disabled}>
		<legend>Reorder column</legend>
		<TargetPicker
			{session}
			{media}
			scope={{ kind: 'Columns' }}
			label="Place after column"
			emptyLabel="First column"
			selected={after}
			exclude={{ kind: 'Column', column: column.id }}
			{disabled}
			choose={(value) => {
				after = value;
			}} />
		<button>Move column</button>
	</fieldset>
</form>
