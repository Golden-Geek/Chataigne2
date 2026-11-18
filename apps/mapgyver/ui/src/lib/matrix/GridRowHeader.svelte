<script lang="ts">
	import { appState, selectOnPress, viewportContour } from 'golden_ui';
	import { sendPatchMetaIntent } from 'golden_ui/store/ui-intents';
	import Arrow from 'golden_ui/components/common/Arrow.svelte';
	import groupIcon from 'golden_ui/style/icons/node/folder.svg';
	import GridLabel from './GridLabel.svelte';
	import RowControls from './RowControls.svelte';
	import ParameterTrigger from './ParameterTrigger.svelte';
	import type { MatrixViewRow } from '../media/generated/MatrixViewRow';
	import { gridSize } from './viewport';
	import { rowInset } from './group-rails';
	let {
		row,
		left,
		disabled,
		selected,
		color,
		identity,
		select,
		rename,
		startDrag
	}: {
		row: MatrixViewRow;
		left: number;
		disabled: boolean;
		selected: boolean;
		color: string | null;
		identity: string;
		select(event: PointerEvent | MouseEvent | KeyboardEvent): void;
		rename(label: string): Promise<boolean>;
		startDrag(event: DragEvent): void;
	} = $props();
	function toggle() {
		const node = appState.session?.graph.state.nodesById.getByUuid(row.item.id.id);
		if (!node || disabled) return;
		void sendPatchMetaIntent(node.node_id, {
			presentation: {
				...node.meta.presentation,
				collapsed: !row.collapsed
			}
		});
	}
</script>

<div
	class="row-label"
	class:group={row.item.id.kind === 'Group'}
	data-matrix-drop="Item"
	data-row-id={row.item.id.id}
	draggable={!disabled}
	ondragstart={startDrag}
	role="rowheader"
	tabindex="0"
	aria-label={row.item.label}
	aria-selected={selected}
	aria-expanded={row.item.id.kind === 'Group' ? !row.collapsed : undefined}
	style:--matrix-item-color={color}
	class:custom-color={color != null}
	use:selectOnPress={{ disabled, select }}
	use:viewportContour={{ selected, enabled: row.item.settings.enabled, identity }}
	style:left={`${left + rowInset(row.depth)}rem`}
	style:width={`${gridSize.label - rowInset(row.depth)}rem`}
	title={row.item.label}>
	<div class="row-title">
		{#if row.item.id.kind === 'Group'}
			<button
				class="group-toggle"
				aria-label={`${row.collapsed ? 'Expand' : 'Collapse'} ${row.item.label}`}
				{disabled}
				onclick={(event) => {
					event.stopPropagation();
					toggle();
				}}>
				<Arrow direction={row.collapsed ? 'right' : 'down'} color="currentColor" />
			</button>
			<img src={groupIcon} alt="" />
		{/if}
		<div class="row-title-label">
			<GridLabel label={row.item.label} {disabled} {select} {rename} />
		</div>
		<ParameterTrigger
			owner={row.item.id.id}
			field="stop"
			label={`Stop ${row.item.label}`}
			{disabled} />
	</div>
	{#if row.item.id.kind === 'Layer'}<RowControls
			uuid={row.item.id.id}
			label={row.item.label} />{/if}
</div>

<style>
	.row-label {
		position: absolute;
		z-index: 2;
		box-sizing: border-box;
		width: var(--label);
		height: calc(var(--row) - 0.3rem);
		background: var(--gc-color-header);
		padding: 0.5rem;
		overflow: hidden;
		display: flex;
		flex-direction: column;
		justify-content: flex-start;
		gap: 0.3rem;
		border: 0.05rem solid var(--gc-color-border);
	}
	.row-label img {
		width: 1.2em;
		height: 1.2em;
	}
	.row-title {
		display: flex;
		align-items: flex-start;
		gap: 0.25rem;
		min-inline-size: 0;
		box-sizing: border-box;
	}
	.row-title-label {
		flex: 1;
		min-inline-size: 0;
	}
	.row-label.custom-color {
		background: color-mix(in srgb, var(--matrix-item-color) 18%, var(--gc-color-header));
	}
	.row-label.group {
		height: var(--row);
		padding-block: 0.15rem;
		justify-content: center;
		background: color-mix(
			in srgb,
			var(--matrix-item-color, var(--gc-color-accent)) 12%,
			var(--gc-color-header)
		);
	}
	.row-label.group::before {
		content: '';
		position: absolute;
		inset: 0.3rem auto 0.3rem 0.15rem;
		width: 0.12rem;
		border-radius: 1rem;
		background: var(--matrix-item-color, var(--gc-color-accent));
	}
	.group .row-title {
		align-items: center;
		flex-wrap: nowrap;
	}
	.group-toggle {
		display: inline-flex;
		flex: none;
		padding: 0;
		border: 0;
		background: transparent;
	}
</style>
