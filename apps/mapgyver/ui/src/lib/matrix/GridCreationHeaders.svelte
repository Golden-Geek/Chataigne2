<script lang="ts">
	import { IconButton } from 'golden_ui';
	import addIcon from 'golden_ui/style/icons/node/add.svg';
	import { gridSize, type projectGrid } from './viewport';
	let {
		projection,
		rows,
		columns,
		disabled,
		menu
	}: {
		projection: ReturnType<typeof projectGrid>;
		rows: number;
		columns: number;
		disabled: boolean;
		menu(event: MouseEvent, axis: 'Column' | 'Layer'): void;
	} = $props();
</script>

<div
	class="creation column"
	role="columnheader"
	style:left={`${projection.columnOrigin + columns * projection.column}rem`}
	style:top={`${projection.top + projection.groupHeight}rem`}
	style:width={`${gridSize.creation}rem`}
	style:height={`${gridSize.header}rem`}>
	<IconButton
		icon={addIcon}
		label="Add Column or Column Group"
		{disabled}
		onclick={(event) => menu(event, 'Column')} />
</div>
<div
	role="row"
	class="creation-row"
	style:top={`${projection.rowOrigin + rows * projection.row}rem`}>
	<div
		class="creation layer"
		role="rowheader"
		style:left={`${projection.left}rem`}
		style:width={`${gridSize.label}rem`}
		style:height={`${gridSize.creation}rem`}>
		<IconButton
			icon={addIcon}
			label="Add Layer or Layer Group"
			{disabled}
			onclick={(event) => menu(event, 'Layer')} />
	</div>
</div>

<style>
	.creation-row {
		position: absolute;
		width: 100%;
	}
	.creation {
		position: absolute;
		display: flex;
		align-items: center;
		justify-content: center;
		box-sizing: border-box;
		background: var(--gc-color-header);
		border: 0.05rem solid var(--gc-color-border);
	}
	.column {
		z-index: 3;
	}
	.layer {
		z-index: 2;
	}
</style>
