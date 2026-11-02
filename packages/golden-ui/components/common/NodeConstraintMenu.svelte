<script lang="ts">
	import { slide } from 'svelte/transition';
	import { sendSetParamConstraintsIntent } from '../../store/ui-intents';
	import type {
		NodeId,
		ParamConstraintPolicy,
		ParamValue,
		UiNodeDto,
		UiParamConstraints,
		UiRangeConstraint
	} from '../../types';
	import type { ContextMenuItem } from './context-menu';

	let {
		visible,
		resolveNode,
		positionPanel,
		onclose
	}: {
		visible: boolean;
		resolveNode: (node: NodeId) => UiNodeDto | null;
		positionPanel: (panel: HTMLElement, anchor: HTMLElement | null) => void;
		onclose: () => void;
	} = $props();

	let rangeSubMenuDiv = $state<HTMLDivElement | null>(null);
	let rangeAnchorElement = $state<HTMLElement | null>(null);
	let optionsSubMenuDiv = $state<HTMLDivElement | null>(null);
	let optionsAnchorElement = $state<HTMLElement | null>(null);
	let rangeSubMenu = $state({ open: false });
	let rangeDraft = $state<{ min: number | null; max: number | null }>({
		min: null,
		max: null
	});
	let rangeTargetNodeId = $state<NodeId | null>(null);
	let optionsSubMenu = $state({ open: false });
	let optionsDraft = $state('');
	let optionsTargetNodeId = $state<NodeId | null>(null);

	const closeRangeSubMenu = (): void => {
		rangeSubMenu.open = false;
		rangeAnchorElement = null;
		rangeTargetNodeId = null;
	};

	const closeOptionsSubMenu = (): void => {
		optionsSubMenu.open = false;
		optionsAnchorElement = null;
		optionsTargetNodeId = null;
	};

	export const close = (): void => {
		closeRangeSubMenu();
		closeOptionsSubMenu();
	};
	export const reposition = (): void => {
		positionSubPanel(rangeSubMenuDiv, rangeAnchorElement);
		positionSubPanel(optionsSubMenuDiv, optionsAnchorElement);
	};
	export const insideElements = (): (HTMLElement | null)[] => [rangeSubMenuDiv, optionsSubMenuDiv];
	export const menuItems = (node: UiNodeDto): ContextMenuItem[] => buildConstraintMenuItems(node);
	const positionSubPanel = (
		panelDiv: HTMLDivElement | null,
		anchorEl: HTMLElement | null
	): void => {
		if (!panelDiv || !anchorEl) return;
		positionPanel(panelDiv, anchorEl);
	};

	const captureSubPanelContainer = (
		setter: (div: HTMLDivElement | null) => void,
		anchorGetter: () => HTMLElement | null
	) => {
		return (node: HTMLDivElement) => {
			setter(node);
			const raf = requestAnimationFrame(() => positionSubPanel(node, anchorGetter()));
			return () => {
				cancelAnimationFrame(raf);
				setter(null);
			};
		};
	};

	const openRangeSubMenu = (
		event: MouseEvent,
		nodeId: NodeId,
		constraints: UiParamConstraints
	): void => {
		rangeTargetNodeId = nodeId;
		if (constraints.range?.kind === 'uniform') {
			rangeDraft.min = constraints.range.min ?? null;
			rangeDraft.max = constraints.range.max ?? null;
		} else {
			rangeDraft.min = null;
			rangeDraft.max = null;
		}
		rangeAnchorElement = event.currentTarget as HTMLElement;
		rangeSubMenu.open = true;
		queueMicrotask(() => positionSubPanel(rangeSubMenuDiv, rangeAnchorElement));
	};

	const applyRange = (): void => {
		if (rangeTargetNodeId === null) return;
		const min = rangeDraft.min ?? undefined;
		const max = rangeDraft.max ?? undefined;
		if (min !== undefined && !Number.isFinite(min)) return;
		if (max !== undefined && !Number.isFinite(max)) return;
		updateNodeConstraints(rangeTargetNodeId, (draft) => {
			draft.range =
				min !== undefined || max !== undefined ? { kind: 'uniform', min, max } : undefined;
		});
		onclose();
	};

	const parseEnumOptions = (text: string): import('../../types').ParamEnumOption[] => {
		const lines = text
			.split('\n')
			.map((l) => l.trim())
			.filter((l) => l.length > 0);
		if (lines.length === 0) {
			return [
				{
					variant_id: 'option',
					value: { kind: 'enum', value: 'option' },
					label: 'Option',
					tags: []
				}
			];
		}
		return lines.map((line) => {
			const eqIdx = line.indexOf('=');
			if (eqIdx !== -1) {
				const label = line.slice(0, eqIdx).trim();
				const valueId = line.slice(eqIdx + 1).trim() || label;
				return {
					variant_id: valueId,
					value: { kind: 'enum' as const, value: valueId },
					label,
					tags: []
				};
			}
			return {
				variant_id: line,
				value: { kind: 'enum' as const, value: line },
				label: line,
				tags: []
			};
		});
	};

	const openOptionsSubMenu = (
		event: MouseEvent,
		nodeId: NodeId,
		constraints: UiParamConstraints
	): void => {
		optionsTargetNodeId = nodeId;
		const opts = constraints.enum_options ?? [];
		optionsDraft = opts
			.map((opt) => (opt.variant_id !== opt.label ? `${opt.label}=${opt.variant_id}` : opt.label))
			.join('\n');
		optionsAnchorElement = event.currentTarget as HTMLElement;
		optionsSubMenu.open = true;
		queueMicrotask(() => positionSubPanel(optionsSubMenuDiv, optionsAnchorElement));
	};

	const applyOptions = (): void => {
		if (optionsTargetNodeId === null) return;
		const options = parseEnumOptions(optionsDraft);
		updateNodeConstraints(optionsTargetNodeId, (draft) => {
			draft.enum_options = options;
		});
		onclose();
	};

	const numericConstraintKinds = new Set<ParamValue['kind']>([
		'int',
		'float',
		'css_value',
		'vec2',
		'vec3',
		'color'
	]);
	const uniformRangePresets: readonly { id: string; label: string; min: number; max: number }[] = [
		{ id: 'zero-one', label: '0 to 1', min: 0, max: 1 },
		{ id: 'minus-one-one', label: '-1 to 1', min: -1, max: 1 },
		{ id: 'zero-hundred', label: '0 to 100', min: 0, max: 100 },
		{ id: 'zero-255', label: '0 to 255', min: 0, max: 255 },
		{ id: 'zero-360', label: '0 to 360', min: 0, max: 360 },
		{ id: 'minus-180-180', label: '-180 to 180', min: -180, max: 180 },
		{ id: 'minus-90-90', label: '-90 to 90', min: -90, max: 90 }
	];
	const integerStepPresets = [1, 2, 5, 10];
	const fractionalStepPresets = [0.001, 0.01, 0.1, 1, 5, 10];

	const isNumericConstraintKind = (kind: ParamValue['kind']): boolean => {
		return numericConstraintKinds.has(kind);
	};

	const cloneConstraints = (constraints: UiParamConstraints): UiParamConstraints => {
		return JSON.parse(JSON.stringify(constraints)) as UiParamConstraints;
	};

	const sanitizeNumber = (value: number | undefined): number | undefined => {
		return typeof value === 'number' && Number.isFinite(value) ? value : undefined;
	};

	const sanitizeNumberList = (values: number[] | undefined): number[] | undefined => {
		if (!Array.isArray(values)) {
			return undefined;
		}
		const sanitized = values.filter((value) => typeof value === 'number' && Number.isFinite(value));
		return sanitized.length > 0 ? sanitized : undefined;
	};

	const normalizeRangeConstraint = (
		range: UiRangeConstraint | undefined
	): UiRangeConstraint | undefined => {
		if (!range) {
			return undefined;
		}
		if (range.kind === 'uniform') {
			const min = sanitizeNumber(range.min);
			const max = sanitizeNumber(range.max);
			if (min === undefined && max === undefined) {
				return undefined;
			}
			return {
				kind: 'uniform',
				min,
				max
			};
		}
		const min = sanitizeNumberList(range.min);
		const max = sanitizeNumberList(range.max);
		if (min === undefined && max === undefined) {
			return undefined;
		}
		return {
			kind: 'components',
			min,
			max
		};
	};

	const finalizeConstraints = (constraints: UiParamConstraints): UiParamConstraints => {
		const next = cloneConstraints(constraints);
		next.range = normalizeRangeConstraint(next.range);
		next.step = sanitizeNumber(next.step);
		if (next.step !== undefined && next.step <= 0) {
			next.step = undefined;
		}
		next.step_base = next.step !== undefined ? sanitizeNumber(next.step_base) : undefined;
		next.policy = next.policy ?? 'ClampAdapt';
		return next;
	};

	const sameNumber = (a: number | undefined, b: number | undefined): boolean => {
		if (a === undefined || b === undefined) {
			return a === b;
		}
		return Math.abs(a - b) <= 1e-9;
	};

	const currentNodeById = (nodeId: NodeId): UiNodeDto | null => {
		return resolveNode(nodeId);
	};

	const updateNodeConstraints = (
		nodeId: NodeId,
		transform: (draft: UiParamConstraints) => void
	): void => {
		const node = currentNodeById(nodeId);
		if (!node || node.data.kind !== 'parameter') {
			return;
		}
		const draft = cloneConstraints(node.data.param.constraints);
		transform(draft);
		void sendSetParamConstraintsIntent(nodeId, finalizeConstraints(draft));
	};

	const hasCurrentUniformRange = (
		constraints: UiParamConstraints,
		min: number,
		max: number
	): boolean => {
		return (
			constraints.range?.kind === 'uniform' &&
			sameNumber(constraints.range.min, min) &&
			sameNumber(constraints.range.max, max)
		);
	};

	const resettableNumericConstraints = (constraints: UiParamConstraints): boolean => {
		return (
			constraints.range !== undefined ||
			constraints.step !== undefined ||
			constraints.step_base !== undefined ||
			constraints.policy === 'Reject'
		);
	};

	const buildConstraintMenuItems = (node: UiNodeDto): ContextMenuItem[] => {
		if (node.data.kind !== 'parameter') {
			return [];
		}

		const paramKind = node.data.param.value.kind;
		const constraints = node.data.param.constraints;
		const supportsNumericConstraints = isNumericConstraintKind(paramKind);
		const isEnumKind = paramKind === 'enum';
		const stepPresets = paramKind === 'int' ? integerStepPresets : fractionalStepPresets;
		const items: ContextMenuItem[] = [];

		if (isEnumKind) {
			items.push({
				id: 'constraint-enum-options',
				label: 'Set Options...',
				className: optionsSubMenu.open ? 'gc-node-context-item-open' : '',
				action: (event) => {
					if (optionsSubMenu.open) {
						closeOptionsSubMenu();
					} else {
						openOptionsSubMenu(event, node.node_id, constraints);
					}
				}
			});
		}

		if (supportsNumericConstraints) {
			items.push({
				id: 'constraint-range',
				label: 'Range',
				submenu: [
					...uniformRangePresets.map((preset) => ({
						id: `constraint-range-${preset.id}`,
						label: preset.label,
						hint: hasCurrentUniformRange(constraints, preset.min, preset.max)
							? 'Current'
							: undefined,
						action: () => {
							updateNodeConstraints(node.node_id, (draft) => {
								draft.range = {
									kind: 'uniform',
									min: preset.min,
									max: preset.max
								};
							});
						}
					})),
					{ separator: true },
					{
						id: 'constraint-range-custom',
						label: 'Custom Range...',
						className: rangeSubMenu.open ? 'gc-node-context-item-open' : '',
						action: (event) => {
							if (rangeSubMenu.open) {
								closeRangeSubMenu();
							} else {
								openRangeSubMenu(event, node.node_id, constraints);
							}
						}
					},
					{ separator: true },
					{
						id: 'constraint-range-clear',
						label: 'Clear Range',
						disabled: constraints.range === undefined,
						action: () => {
							updateNodeConstraints(node.node_id, (draft) => {
								draft.range = undefined;
							});
						}
					}
				]
			});
			items.push({
				id: 'constraint-step',
				label: 'Step',
				submenu: [
					...stepPresets.map((step) => ({
						id: `constraint-step-${String(step).replace('.', '_')}`,
						label: `${step}`,
						hint: sameNumber(constraints.step, step) ? 'Current' : undefined,
						action: () => {
							updateNodeConstraints(node.node_id, (draft) => {
								draft.step = step;
							});
						}
					})),
					{ separator: true },
					{
						id: 'constraint-step-clear',
						label: 'Clear Step',
						disabled: constraints.step === undefined,
						action: () => {
							updateNodeConstraints(node.node_id, (draft) => {
								draft.step = undefined;
								draft.step_base = undefined;
							});
						}
					}
				]
			});
			items.push({
				id: 'constraint-policy',
				label: 'Constraint Policy',
				submenu: (
					[
						['ClampAdapt', 'Clamp / Adapt'],
						['Reject', 'Reject']
					] as const
				).map(([policy, label]) => ({
					id: `constraint-policy-${policy}`,
					label,
					hint: constraints.policy === policy ? 'Current' : undefined,
					action: () => {
						updateNodeConstraints(node.node_id, (draft) => {
							draft.policy = policy satisfies ParamConstraintPolicy;
						});
					}
				}))
			});
		}

		if (supportsNumericConstraints && resettableNumericConstraints(constraints)) {
			items.push({ separator: true });
		}

		if (resettableNumericConstraints(constraints)) {
			items.push({
				id: 'constraint-reset',
				label: supportsNumericConstraints ? 'Reset Numeric Constraints' : 'Reset Constraints',
				action: () => {
					updateNodeConstraints(node.node_id, (draft) => {
						draft.range = undefined;
						draft.step = undefined;
						draft.step_base = undefined;
						draft.policy = 'ClampAdapt';
					});
				}
			});
		}

		if (items.length === 0) {
			return [
				{
					id: 'constraint-unavailable',
					label: 'No editable constraints here',
					disabled: true
				}
			];
		}

		return items;
	};

	let showRangeSubMenu = $derived(visible && rangeSubMenu.open && rangeAnchorElement !== null);
	let showOptionsSubMenu = $derived(
		visible && optionsSubMenu.open && optionsAnchorElement !== null
	);
	$effect(() => {
		if (!visible) close();
	});
</script>

{#if showRangeSubMenu}
	<div
		class="gc-node-context-side-panel"
		{@attach captureSubPanelContainer(
			(d) => {
				rangeSubMenuDiv = d;
			},
			() => rangeAnchorElement
		)}
		transition:slide={{ axis: 'x', duration: 150 }}>
		<div class="gc-constraint-editor">
			<div class="gc-constraint-editor-title">Custom Range</div>
			<label class="gc-constraint-editor-field">
				<span>Min</span>
				<input
					type="number"
					placeholder="none"
					bind:value={rangeDraft.min}
					onkeydown={(e) => {
						if (e.key === 'Enter') applyRange();
					}} />
			</label>
			<label class="gc-constraint-editor-field">
				<span>Max</span>
				<input
					type="number"
					placeholder="none"
					bind:value={rangeDraft.max}
					onkeydown={(e) => {
						if (e.key === 'Enter') applyRange();
					}} />
			</label>
			<button class="gc-constraint-editor-apply" onclick={applyRange}>Apply</button>
		</div>
	</div>
{/if}

{#if showOptionsSubMenu}
	<div
		class="gc-node-context-side-panel"
		{@attach captureSubPanelContainer(
			(d) => {
				optionsSubMenuDiv = d;
			},
			() => optionsAnchorElement
		)}
		transition:slide={{ axis: 'x', duration: 150 }}>
		<div class="gc-constraint-editor">
			<div class="gc-constraint-editor-title">Enum Options</div>
			<div class="gc-constraint-editor-hint">
				One option per line. Use Label=value for a custom id.
			</div>
			<textarea
				class="gc-constraint-editor-textarea"
				rows={6}
				placeholder={'Option 1\nOption 2=value2'}
				bind:value={optionsDraft}
				onkeydown={(e) => {
					if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) applyOptions();
				}}></textarea>
			<button class="gc-constraint-editor-apply" onclick={applyOptions}>Apply</button>
		</div>
	</div>
{/if}

<style>
	.gc-node-context-side-panel {
		position: fixed;
		left: -10000px;
		top: -10000px;
		z-index: 1301;
		background: var(--gc-color-surface-2, #1e1e1e);
		border: 0.0625rem solid var(--gc-color-border, #444);
		border-radius: 0.4rem;
		box-shadow: 0 0.25rem 1rem rgba(0, 0, 0, 0.4);
		overflow: auto;
		overscroll-behavior: contain;
		box-sizing: border-box;
	}

	.gc-constraint-editor {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		padding: 0.65rem 0.75rem;
		min-inline-size: min(13rem, var(--gc-node-context-panel-max-inline-size, calc(100vw - 0.9rem)));
	}

	.gc-constraint-editor-title {
		font-size: 0.72rem;
		font-weight: 600;
		color: var(--gc-color-text-secondary, #aaa);
		text-transform: uppercase;
		letter-spacing: 0.05em;
		margin-block-end: 0.1rem;
	}

	.gc-constraint-editor-hint {
		font-size: 0.68rem;
		color: var(--gc-color-text-secondary, #888);
		line-height: 1.3;
	}

	.gc-constraint-editor-field {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		font-size: 0.75rem;

		span {
			flex: 0 0 2.5rem;
			color: var(--gc-color-text-secondary, #aaa);
		}

		input {
			flex: 1;
			background: var(--gc-color-surface-1, #141414);
			border: 1px solid var(--gc-color-border, #444);
			border-radius: 0.25rem;
			color: var(--gc-color-text, #eee);
			font: inherit;
			font-size: 0.75rem;
			padding: 0.22rem 0.4rem;
			outline: none;
			min-inline-size: 0;

			&:focus {
				border-color: var(--gc-color-accent, #6c8ebf);
			}
		}
	}

	.gc-constraint-editor-textarea {
		resize: vertical;
		background: var(--gc-color-surface-1, #141414);
		border: 1px solid var(--gc-color-border, #444);
		border-radius: 0.25rem;
		color: var(--gc-color-text, #eee);
		font: inherit;
		font-size: 0.75rem;
		padding: 0.3rem 0.4rem;
		outline: none;
		min-inline-size: 0;
		width: 100%;
		box-sizing: border-box;
		font-family: monospace;

		&:focus {
			border-color: var(--gc-color-accent, #6c8ebf);
		}
	}

	.gc-constraint-editor-apply {
		align-self: flex-end;
		background: var(--gc-color-accent, #6c8ebf);
		border: none;
		border-radius: 0.25rem;
		color: white;
		cursor: pointer;
		font: inherit;
		font-size: 0.72rem;
		font-weight: 600;
		padding: 0.25rem 0.75rem;

		&:hover {
			filter: brightness(1.15);
		}
	}
</style>
