<script lang="ts">
	import { untrack } from 'svelte';
	import type { WorkbenchSession } from 'golden_ui';
	import type { MatrixViewRow } from '../media/generated/MatrixViewRow';
	import type { MatrixEdit } from '../media/generated/MatrixEdit';
	import type { MatrixTargetId } from '../media/generated/MatrixTargetId';
	import type { BlendMode } from '../media/generated/BlendMode';
	import NameForm from './NameForm.svelte';
	import TargetPicker from './TargetPicker.svelte';
	let {
		row,
		media,
		session,
		disabled,
		edit
	}: {
		row: MatrixViewRow;
		media: string;
		session: WorkbenchSession | null;
		disabled: boolean;
		edit(value: MatrixEdit): unknown;
	} = $props();
	let parent = $state(untrack(() => row.item.parent));
	let after = $state<MatrixTargetId | null>(null);
	const blends: BlendMode[] = ['Normal', 'Add', 'Multiply', 'Screen'];
</script>

<NameForm
	label="Row name"
	value={row.item.label}
	{disabled}
	rename={(label) => edit({ kind: 'RenameItem', item: row.item.id, label })} />
<form
	class="matrix-authoring"
	onsubmit={(event) => {
		event.preventDefault();
		const data = new FormData(event.currentTarget);
		edit({
			kind: 'ConfigureItem',
			item: row.item.id,
			settings: {
				opacity: Number(data.get('opacity')),
				blend: String(data.get('blend')) as BlendMode,
				enabled: data.has('enabled')
			}
		});
	}}>
	<fieldset {disabled}>
		<legend>Row blending</legend>
		<label
			>Opacity<input
				name="opacity"
				type="number"
				min="0"
				max="1"
				step="any"
				required
				value={row.item.settings.opacity} /></label>
		<label
			>Blend<select name="blend" value={row.item.settings.blend}
				>{#each blends as blend}<option>{blend}</option>{/each}</select
			></label>
		<label
			><input type="checkbox" name="enabled" checked={row.item.settings.enabled} />Enabled</label>
		<button>Save row settings</button>
	</fieldset>
</form>
<form
	class="matrix-authoring"
	onsubmit={(event) => {
		event.preventDefault();
		edit({
			kind: 'MoveItem',
			item: row.item.id,
			parent,
			after: after?.kind === 'Item' ? after.item : null
		});
	}}>
	<fieldset {disabled}>
		<legend>Move row</legend>
		<TargetPicker
			{session}
			{media}
			scope={{ kind: 'Groups' }}
			label="Destination group"
			emptyLabel="Root"
			selected={parent ? { kind: 'Item', item: { kind: 'Group', id: parent } } : null}
			exclude={{ kind: 'Item', item: row.item.id }}
			{disabled}
			choose={(value) => {
				parent = value?.kind === 'Item' && value.item.kind === 'Group' ? value.item.id : null;
				after = null;
			}} />
		<TargetPicker
			{session}
			{media}
			scope={{ kind: 'Items', parent }}
			label="Place after row"
			emptyLabel="First in destination"
			selected={after}
			exclude={{ kind: 'Item', item: row.item.id }}
			{disabled}
			choose={(value) => {
				after = value;
			}} />
		<button>Move row</button>
	</fieldset>
</form>
{#if row.item.id.kind === 'Group'}
	<div class="matrix-authoring actions">
		<button
			{disabled}
			onclick={() => {
				if (row.item.id.kind === 'Group')
					edit({ kind: 'AddLayers', parent: row.item.id.id, count: 1 });
			}}>Add child layer</button>
		<button
			{disabled}
			onclick={() => {
				if (row.item.id.kind === 'Group')
					edit({ kind: 'AddGroup', parent: row.item.id.id, label: null });
			}}>Add child group</button>
	</div>
{/if}
