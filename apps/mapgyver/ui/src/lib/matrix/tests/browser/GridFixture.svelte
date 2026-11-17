<script lang="ts">
	import { onMount } from 'svelte';
	import { appState, nodeSelectionMode, type UiNodeDto, type ParamValue } from 'golden_ui';
	import { createFixtureSession, fixtureNode } from 'golden_ui/testing/session';
	import VirtualGrid from '../../VirtualGrid.svelte';
	import { viewportResult } from '../fixture';
	import type { MatrixSelection } from '../../../media/generated/MatrixSelection';
	import type { ColumnSnapshot } from '../../../media/generated/ColumnSnapshot';
	import type { MatrixViewRow } from '../../../media/generated/MatrixViewRow';
	let { onselect }: { onselect(id: string): void } = $props();
	const fixture = createFixtureSession();
	const result = viewportResult('matrix');
	if (result.kind !== 'MatrixViewport') throw new Error('Expected Matrix viewport fixture');
	const snapshot = result.snapshot;
	snapshot.row_total = snapshot.column_total = snapshot.axis_total = 2;
	snapshot.axis_depth = 1;
	snapshot.columns = ['Column A', 'Column B'].map((label, index) => ({
		id: `column-${index}`,
		axis_index: index,
		parent: 'group',
		label,
		definition: { transition: null },
		application: `column-app-${index}`
	}));
	snapshot.column_groups = [
		{
			id: 'group',
			parent: null,
			ordinal: 0,
			depth: 0,
			axis_start: 0,
			axis_count: 2,
			empty: false,
			label: 'Group A',
			application: 'group-app',
			column_start: 0,
			column_count: 2
		}
	];
	snapshot.rows = ['Layer A', 'Layer B'].map((label, index) => ({
		depth: 0,
		item: {
			id: { kind: 'Layer', id: `layer-${index}` },
			parent: null,
			label,
			settings: { expose_channel: false, enabled: true, opacity: 1, blend: 'Normal' },
			layer: { position: [0, 0], size: { width: 400, height: 300 }, duration_ms: 1000 },
			channel: { media: 'matrix', channel: `layer-output-${index}` }
		}
	}));
	snapshot.rows.unshift({
		depth: 0,
		item: {
			id: { kind: 'Group', id: 'layer-group' },
			parent: null,
			label: 'Layer Group',
			settings: { expose_channel: false, enabled: true, opacity: 1, blend: 'Normal' },
			layer: null,
			channel: { media: 'matrix', channel: 'group-output' }
		}
	});
	snapshot.rows[1].depth = 1;
	snapshot.rows[1].item.parent = 'layer-group';
	snapshot.rows[2].item.settings.enabled = false;
	snapshot.row_total = 3;
	snapshot.cells = snapshot.rows
		.filter((row) => row.item.id.kind === 'Layer')
		.flatMap((row) =>
			snapshot.columns.map((column) => ({
				cell: {
					id: `cell-${row.item.id.id}-${column.id}`,
					media: null,
					layer: row.item.id.id,
					definition: {
						column: column.id,
						source: null,
						application: 'cell-app',
						transition: null,
						settings: { position: [0, 0], size: null, fit_mode: 'Fit', opacity: 1, enabled: true }
					}
				},
				source_label: null,
				source_preview: null,
				source_missing: true
			}))
		);
	const nodes: UiNodeDto[] = [fixtureNode(0, 'Project', [1])];
	const byUuid = new Map<string, UiNodeDto>();
	function node(uuid: string, label: string, parent: UiNodeDto) {
		const value = fixtureNode(nodes.length, label);
		value.uuid = uuid;
		parent.children.push(value.node_id);
		nodes.push(value);
		byUuid.set(uuid, value);
		return value;
	}
	const matrix = fixtureNode(1, 'Matrix');
	matrix.uuid = 'matrix';
	nodes.push(matrix);
	byUuid.set('matrix', matrix);
	for (const item of [
		snapshot.column_groups[0],
		...snapshot.columns,
		...snapshot.rows.map((row) => ({
			id: row.item.id.id,
			label: row.item.label,
			parent: row.item.parent
		}))
	]) {
		const owner = node(item.id, item.label, item.parent ? byUuid.get(item.parent)! : matrix);
		const parameters = node(`${item.id}-parameters`, 'Parameters', owner);
		parameters.decl_id = 'parameters';
		const stop = node(`${item.id}-stop`, 'Stop', parameters);
		stop.decl_id = 'parameters/stop';
		const value: ParamValue = { kind: 'trigger' };
		stop.node_type = 'parameter';
		stop.data = {
			kind: 'parameter',
			param: {
				value,
				default_value: value,
				read_only: false,
				event_behaviour: 'Coalesce',
				constraints: { enum_options: [], policy: 'ClampAdapt' },
				ui_hints: {},
				control: { mode: 'manual', spec: { mode: 'manual' } },
				reference_allowed_targets: [],
				reference_visible_nodes: []
			}
		};
	}
	for (const cell of snapshot.cells) node(cell.cell.id, 'Cell', byUuid.get(cell.cell.layer)!);
	fixture.loadNodes(nodes);
	// The shared fixture deliberately issues fresh UUIDs on load. Project them into app DTOs.
	const uuid = (alias: string) => byUuid.get(alias)!.uuid;
	const aliases = new Map([...byUuid].map(([alias, value]) => [value.uuid, alias]));
	const byActual = new Map(nodes.map((value) => [value.uuid, value]));
	snapshot.media = uuid('matrix');
	snapshot.final_output.reference.media = snapshot.media;
	snapshot.rows = snapshot.rows.map((row) => ({
		...row,
		item: {
			...row.item,
			channel: { ...row.item.channel, media: snapshot.media },
			parent: row.item.parent ? uuid(row.item.parent) : null,
			id: { kind: row.item.id.kind, id: uuid(row.item.id.id) }
		}
	}));
	snapshot.columns = snapshot.columns.map((column) => ({
		...column,
		id: uuid(column.id),
		parent: uuid('group')
	}));
	snapshot.column_groups = snapshot.column_groups.map((group) => ({
		...group,
		id: uuid(group.id)
	}));
	snapshot.cells = snapshot.cells.map((value) => ({
		...value,
		cell: {
			...value.cell,
			id: uuid(value.cell.id),
			layer: uuid(value.cell.layer),
			definition: { ...value.cell.definition, column: uuid(value.cell.definition.column) }
		}
	}));
	let selected = $state<MatrixSelection | null>(null),
		selectedNode = $state<string | null>(null);
	let selections = $state(0),
		clears = $state(0),
		mutations = $state(0),
		launches = $state(0);
	let disabled = $state(false),
		comparison = $state(false);
	function choose(uuid: string, event?: PointerEvent | MouseEvent | KeyboardEvent) {
		selectedNode = uuid;
		selections++;
		fixture.session.selectNode(
			byActual.get(uuid)!.node_id,
			event ? nodeSelectionMode(event) : 'REPLACE'
		);
		onselect(aliases.get(uuid)!);
	}
	function cell(
		row: MatrixViewRow,
		column: ColumnSnapshot,
		event: PointerEvent | MouseEvent | KeyboardEvent
	) {
		selected = { item: row.item.id, column: column.id };
		choose(
			snapshot.cells.find(
				(value) => value.cell.layer === row.item.id.id && value.cell.definition.column === column.id
			)!.cell.id,
			event
		);
	}
	fixture.handleIntent((intent) => {
		if (intent.kind === 'patchMeta') return null;
		mutations++;
		return Promise.resolve(fixture.editAcknowledgement());
	});
	onMount(() => {
		const previous = appState.session;
		appState.session = fixture.session;
		void fixture.connect();
		return () => {
			fixture.disconnect();
			appState.session = previous;
		};
	});
</script>

<div class="fixture">
	<div class="receipts">
		<output aria-label="Grid selected node"
			>{selectedNode ? aliases.get(selectedNode) : 'none'}</output>
		<output aria-label="Grid connection"
			>{fixture.session.status}:{fixture.session.loading.visible ? 'loading' : 'ready'}</output>
		<output aria-label="Grid selections">{selections}</output>
		<output aria-label="Grid clears">{clears}</output>
		<output aria-label="Grid mutations">{mutations}</output>
		<output aria-label="Grid launches">{launches}</output>
	</div>
	<div class="actions">
		<button onclick={() => fixture.session.selectNode(byUuid.get('layer-1')!.node_id, 'REPLACE')}
			>Select Layer B externally</button>
		<button
			onclick={async () => {
				for (const alias of [
					'layer-0',
					'layer-group',
					'column-0',
					'group',
					'cell-layer-0-column-0'
				]) {
					const target = byUuid.get(alias)!;
					await fixture.session.client?.sendIntent({
						kind: 'patchMeta',
						node: target.node_id,
						patch: { presentation: { color: { r: 0.9, g: 0.15, b: 0.25, a: 1 } } }
					});
				}
			}}>Set grid custom colors</button>
		<button
			onclick={() => {
				disabled = !disabled;
			}}>Toggle grid disabled</button>
		<button
			onclick={() => {
				comparison = !comparison;
			}}>Toggle independent grid</button>
	</div>
	{#each comparison ? ['A', 'B'] : ['A'] as id (id)}
		<section aria-label={`Grid editor ${id}`} class:comparison={id === 'B'}>
			<VirtualGrid
				{snapshot}
				{selected}
				{disabled}
				observation={null}
				applicationRuntime={null}
				viewport={() => {}}
				select={cell}
				selectRow={(row, event) => {
					selected = { item: row.item.id, column: selected?.column ?? null };
					choose(row.item.id.id, event);
				}}
				selectColumn={(column, event) => {
					selected = { item: selected?.item ?? null, column: column.id };
					choose(column.id, event);
				}}
				selectGroup={(group, event) => {
					selected = null;
					choose(group.id, event);
				}}
				clearSelection={() => {
					selected = null;
					clears++;
					choose(snapshot.media);
				}}
				trigger={() => {
					launches++;
				}}
				drop={() => {}}
				cellSource={() => null}
				cellDrop={() => {}}
				cellCopy={() => false}
				cellPaste={async () => false}
				edit={async () => {
					mutations++;
					return null;
				}}
				menu={() => {}}
				createMenu={() => {}} />
		</section>
	{/each}
</div>

<style>
	.fixture {
		height: 100%;
		display: flex;
		flex-direction: column;
	}
	.receipts {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		line-height: 1.2rem;
	}
	output {
		white-space: nowrap;
		overflow: hidden;
	}
	.actions {
		display: flex;
	}
	section {
		flex: 1;
		min-height: 0;
	}
	.comparison {
		position: fixed;
		inset-block-start: 1rem;
		inset-inline-end: 1rem;
		width: 40vw;
		height: 40vh;
		z-index: 10;
	}
</style>
