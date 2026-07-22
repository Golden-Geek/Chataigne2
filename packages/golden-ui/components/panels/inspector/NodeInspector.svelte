<script lang="ts">
	import { untrack } from 'svelte';
	import { slide, type SlideParams } from 'svelte/transition';
	import type { NodeId, UiNodeDto } from '../../../types';
	import Self from './NodeInspector.svelte';
	import { appState } from '../../../store/workbench.svelte';
	import { sendPatchMetaIntent } from '../../../store/ui-intents';
	import ParameterInspector from './ParameterInspector.svelte';
	import { getContainerColorForNode, getIconURLForNode } from '../../../store/node-types';
	import EnableButton from '../../common/EnableButton.svelte';
	import NodeWarningBadge from '../../common/NodeWarningBadge.svelte';
	import { resolveNodeInspector, type NodeInspectorOrder } from './node-inspector-registry';
	import NodeAddButton from '../../common/NodeAddButton.svelte';
	import type { Snippet } from 'svelte';
	import Arrow from '../../common/Arrow.svelte';
	import type { NodePickerModalView } from '../../../store/node-picker-modal.svelte';

	let {
		nodes = [],
		level = 0,
		order = '',
		controlNodeType = '',
		layoutMode = 'default',
		includeChildren = true,
		maxChildLevel = null,
		labelOverride = null,
		density = 'default',
		referencePickerViews = []
	} = $props<{
		nodes: UiNodeDto[];
		level: number;
		order?: NodeInspectorOrder;
		controlNodeType?: String;
		layoutMode?: 'default' | 'dashboard';
		includeChildren?: boolean;
		maxChildLevel?: number | null;
		labelOverride?: string | null;
		density?: 'default' | 'compact';
		referencePickerViews?: NodePickerModalView[];
	}>();

	let session = $derived(appState.session);
	let node = $derived(
		nodes.length > 0 ? (session?.graph.state.nodesById.get(nodes[0]?.node_id) ?? nodes[0]) : null
	);

	const isWithinUserContextScope = (candidate: UiNodeDto | null): boolean => {
		if (!candidate || !session) {
			return false;
		}

		let currentNodeId: NodeId | undefined = candidate.node_id;
		while (currentNodeId !== undefined) {
			const currentNode = session.graph.state.nodesById.get(currentNodeId);
			if (!currentNode) {
				return false;
			}
			if (currentNode.node_type === 'user_context') {
				return true;
			}
			currentNodeId = session.graph.state.parentById.get(currentNodeId);
		}

		return false;
	};

	let showsAllItemRootsInInspector = $derived(isWithinUserContextScope(node));

	const shouldRenderChildInInspector = (child: UiNodeDto): boolean => {
		if (child.meta.presentation?.show_in_inspector_content === false) {
			return false;
		}

		if (showsAllItemRootsInInspector) {
			return true;
		}

		if (child.user_role !== 'itemRoot') {
			return true;
		}

		// Item roots are visible by default; only an explicit opt-out hides them here.
		if (child.meta.presentation?.show_in_nested_inspector !== false) {
			return true;
		}

		return session?.isNodeSelected(child.node_id) ?? false;
	};

	let isRoot = $derived(level === 0);
	let isFirstLevel = $derived(level === 1);

	let color = $derived(node ? getContainerColorForNode(node) : 'rgba(124, 138, 162, 1)');
	let childNodes = $derived.by(() => {
		if (!includeChildren) {
			return [];
		}
		if (maxChildLevel !== null && level >= maxChildLevel) {
			return [];
		}

		return (node?.children ?? [])
			.map((childId: NodeId) => session?.getNodeData(childId))
			.filter((child: UiNodeDto | null | undefined): child is UiNodeDto => child != null)
			.filter((child: UiNodeDto) => shouldRenderChildInInspector(child));
	});
	let isParameter = $derived(node?.data.kind === 'parameter');
	let showAsContainer = $derived(!isParameter);
	let hasChildren = $derived(childNodes.length > 0);
	let canBeDisabled = $derived(node?.meta?.can_be_disabled ?? false);
	let isNameChangeable = $derived(node?.meta?.user_permissions.can_edit_name ?? false);
	let iconURL = $derived(getIconURLForNode(node));
	let warnings = $derived(node ? session?.getNodeVisibleWarnings(node.node_id) : null);
	let customInspectorEntry = $derived(node && showAsContainer ? resolveNodeInspector(node) : null);
	let CustomInspectorComponent = $derived(customInspectorEntry?.component ?? null);

	let defaultCollapsed = $derived(
		Boolean(node?.meta.presentation?.collapsed ?? (level > 3 && !isParameter))
	);
	let collapsedOverride = $state<boolean | null>(null);
	let collapsedOverrideNodeId = $state<NodeId | null>(null);
	let collapsed = $derived(isRoot ? false : (collapsedOverride ?? defaultCollapsed));

	let titleTextElem: HTMLSpanElement | null = $state(null as HTMLSpanElement | null);
	let enableButtonElem: HTMLDivElement | null = $state(null as HTMLDivElement | null);
	let renameInputElem: HTMLInputElement | null = $state(null as HTMLInputElement | null);
	let renamingState = $state({
		isRenaming: false,
		renameDraft: ''
	});

	let creatableItems = $derived(node.creatable_user_items ?? []);
	let showAddButton = $derived(!isRoot && showAsContainer && creatableItems.length > 0);
	const footerHoverToken = Symbol('node-inspector-footer-hover');
	let hoverActive = $state(false);

	$effect(() => {
		if (!renamingState.isRenaming || !renameInputElem) {
			return;
		}

		renameInputElem.focus();
		renameInputElem.select();
	});

	$effect.pre(() => {
		const nodeId = node?.node_id ?? null;
		if (collapsedOverrideNodeId === nodeId) {
			return;
		}
		collapsedOverride = null;
		collapsedOverrideNodeId = nodeId;
	});

	const startRename = (): void => {
		if (!node || !isNameChangeable) {
			return;
		}
		renamingState.renameDraft = node.meta.label;
		renamingState.isRenaming = true;
	};

	const cancelRename = (): void => {
		if (!node) {
			renamingState.isRenaming = false;
			return;
		}
		renamingState.renameDraft = node.meta.label;
		renamingState.isRenaming = false;
	};

	const commitRename = async (): Promise<void> => {
		if (!node || !isNameChangeable) {
			renamingState.isRenaming = false;
			return;
		}

		const nextName = String(renamingState.renameDraft ?? '').trim();
		if (!nextName || nextName === node.meta.label) {
			renamingState.isRenaming = false;
			return;
		}

		await sendPatchMetaIntent(node.node_id, { label: nextName });
		renamingState.isRenaming = false;
	};

	const persistCollapsed = (target: UiNodeDto, nextCollapsed: boolean): void => {
		void sendPatchMetaIntent(target.node_id, {
			presentation: {
				...(target.meta.presentation ?? {}),
				collapsed: nextCollapsed
			}
		});
	};

	const toggleCollapsed = (): void => {
		if (isRoot) {
			return;
		}
		setCollapsed(!collapsed);
	};

	const setCollapsed = (nextCollapsed: boolean): void => {
		if (isRoot) {
			return;
		}
		collapsedOverride = nextCollapsed;
		if (node) {
			persistCollapsed(node, nextCollapsed);
		}
	};

	const handlePointerEnter = (): void => {
		hoverActive = true;
	};

	const handlePointerLeave = (): void => {
		hoverActive = false;
	};

	$effect(() => {
		if (!hoverActive || !node) {
			untrack(() => {
				session?.clearFooterHover(footerHoverToken);
			});
			return;
		}
		untrack(() => {
			session?.setFooterHover(footerHoverToken, node.node_id);
		});
		return () => {
			untrack(() => {
				session?.clearFooterHover(footerHoverToken);
			});
		};
	});
</script>

{#if node}
	<div
		class="node-inspector {node.data.kind} 
			{isRoot ? 'root' : !isFirstLevel ? 'nested' : ''}
		{isParameter ? 'parameter' : 'container'}"
		role="group"
		class:custom-component={!!CustomInspectorComponent}
		class:layout-dashboard={layoutMode === 'dashboard'}
		data-node-id={node.node_id}
		style="--container-color: {color}"
		onpointerenter={handlePointerEnter}
		onpointerleave={handlePointerLeave}>
		{#snippet builtInHeader(headerExtra?: Snippet)}
			{#if !isRoot}
				<div class="node-header">
					<span
						class="node-title"
						role="switch"
						class:name-changeable={isNameChangeable}
						aria-checked={!collapsed}
						tabindex="0"
						onclick={(e) => {
							if (renamingState.isRenaming) {
								return;
							}

							const target = e.target as HTMLElement;
							const clickedTitle = Boolean(
								titleTextElem && target && titleTextElem.contains(target)
							);
							const clickedEnableButton = Boolean(
								enableButtonElem && target && enableButtonElem.contains(target)
							);

							let shouldToggle = !clickedEnableButton;
							if (isNameChangeable) {
								if (clickedTitle) shouldToggle = false;
							}
							if (shouldToggle) {
								toggleCollapsed();
							}
						}}
						onkeydown={(e) => {
							if (renamingState.isRenaming) {
								return;
							}
							if (e.key === 'Enter' || e.key === ' ') {
								toggleCollapsed();
							}
						}}>
						{#if hasChildren}
							<Arrow direction={collapsed ? 'right' : 'down'} />
						{/if}
						<span class="header-icon">
							<img src={iconURL} alt="" />
						</span>

						{#if canBeDisabled}
							<div class="enable-button-wrapper" bind:this={enableButtonElem}>
								<EnableButton {node} />
							</div>
						{/if}

						{#if renamingState.isRenaming}
							<input
								class="node-title-input"
								bind:this={renameInputElem}
								bind:value={renamingState.renameDraft}
								onclick={(e) => {
									e.stopPropagation();
								}}
								onblur={() => {
									void commitRename();
								}}
								onkeydown={(e) => {
									if (e.key === 'Enter') {
										void commitRename();
										return;
									}
									if (e.key === 'Escape') {
										cancelRename();
									}
								}} />
						{:else}
							<span
								class="title-text"
								bind:this={titleTextElem}
								role="textbox"
								tabindex="-1"
								ondblclick={(e) => {
									if (!isNameChangeable) {
										return;
									}
									e.stopPropagation();
									startRename();
								}}
								title={isNameChangeable ? 'Double-click to rename' : undefined}>
								{node.meta.label || 'Container'}
							</span>
						{/if}

						<NodeWarningBadge {warnings} />

						{#if headerExtra}
							{@render headerExtra()}
						{/if}
					</span>

					<span class="spacer"></span>
					{#if showAddButton}
						<div class="node-add-button-wrapper">
							<NodeAddButton {node} />
						</div>
					{/if}
				</div>
			{/if}
		{/snippet}

		{#snippet builtInChildItems(controlNodeType: String = '')}
			{#each childNodes as child, index (child.node_id)}
				<Self
					nodes={[child]}
					level={level + 1}
					order={childNodes.length === 1
						? 'solo'
						: index === 0
							? 'first'
							: index === childNodes.length - 1
								? 'last'
								: ''}
					{layoutMode}
					{includeChildren}
					{maxChildLevel}
					{density}
					{referencePickerViews}
					{controlNodeType} />
			{/each}
		{/snippet}

		{#snippet builtInChildren(controlNodeType: String = '')}
			<div class="node-inspector-children">
				{#if !collapsed}
					<div class="node-inspector-children-wrapper" transition:slide={{ duration: 200 }}>
						{@render builtInChildItems(controlNodeType)}
					</div>
				{/if}
			</div>
		{/snippet}

		{#snippet builtInChildrenStatic(controlNodeType: String = '')}
			<div class="node-inspector-children">
				<div class="node-inspector-children-wrapper">
					{@render builtInChildItems(controlNodeType)}
				</div>
			</div>
		{/snippet}

		{#snippet builtInContent(content?: Snippet, extraClass: String = '')}
			<div class="node-inspector-content {extraClass} {showAddButton ? 'with-add-button' : ''}">
				{#if !collapsed}
					<div class="node-inspector-custom-content-wrapper" transition:slide={{ duration: 200 }}>
						{@render content?.()}
					</div>
				{/if}
			</div>
		{/snippet}

		{#if isParameter}
			<ParameterInspector
				{node}
				{level}
				{order}
				{controlNodeType}
				{layoutMode}
				{labelOverride}
				{density}
				{referencePickerViews}>
				{#snippet defaultChildren(extraClass: String = '')}
					{@render builtInChildren(extraClass)}
				{/snippet}
			</ParameterInspector>
		{:else if CustomInspectorComponent}
			<CustomInspectorComponent
				{node}
				{level}
				{order}
				{includeChildren}
				{maxChildLevel}
				{layoutMode}
				{collapsed}
				{hasChildren}
				{toggleCollapsed}
				{setCollapsed}>
				{#snippet defaultHeader(headerExtra?: Snippet)}
					{@render builtInHeader(headerExtra)}
				{/snippet}
				{#snippet defaultContent(content?: Snippet, extraClass: String = '')}
					{@render builtInContent(content, extraClass)}
				{/snippet}
				{#snippet defaultChildren(extraClass: String = '')}
					{@render builtInChildrenStatic(extraClass)}
				{/snippet}
			</CustomInspectorComponent>
		{:else}
			{@render builtInHeader()}
			<div class="node-inspector-content {showAddButton ? 'with-add-button' : ''}">
				{@render builtInChildren()}
			</div>
		{/if}
	</div>
{/if}

<style>
	.node-inspector {
		display: flex;
		flex-direction: column;
		border-radius: 0.5rem;
		font-size: 0.7rem;
		overflow: visible;
	}

	.node-inspector.nested {
		margin-left: 0.3rem;
	}

	.node-inspector {
		padding-top: 0.6rem;
	}

	.node-inspector.parameter {
		padding-top: 0.5rem;
	}

	.node-inspector.root {
		padding-top: 0.2rem;
		box-sizing: border-box;

		:global(.node-inspector-content) {
			flex: 1;
		}
	}

	.node-inspector.root.custom-component {
		height: 100%;
	}

	.node-inspector.layout-dashboard {
		inline-size: 100%;
		block-size: 100%;
		min-inline-size: 0;
		min-block-size: 0;
		overflow: hidden;
	}

	.node-inspector.layout-dashboard.root {
		padding-top: 0;
	}

	.node-inspector.layout-dashboard .node-header,
	.node-inspector.layout-dashboard .node-title,
	.node-inspector.layout-dashboard .title-text,
	.node-inspector.layout-dashboard .node-inspector-children,
	.node-inspector.layout-dashboard .node-inspector-children-wrapper,
	.node-inspector.layout-dashboard .node-inspector-custom-content-wrapper,
	.node-inspector.layout-dashboard :global(.node-inspector-content) {
		min-inline-size: 0;
	}

	.node-inspector.layout-dashboard .node-title {
		max-inline-size: 100%;
	}

	.node-inspector.layout-dashboard .title-text {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		max-inline-size: 100%;
	}

	.node-inspector.layout-dashboard .node-title-input {
		min-width: 0;
		width: 100%;
	}

	.node-inspector.layout-dashboard :global(.node-inspector-content) {
		flex: 1 1 auto;
		min-block-size: 0;
		overflow: auto;
	}

	.node-inspector.nested {
		padding-top: 0.5rem;
	}

	.node-inspector.nested:not(.parameter) {
		padding-right: 0.2rem;
	}

	.node-inspector:not(.parameter):not(.root) {
		border-top-left-radius: 0.5rem;
		border-top-right-radius: 0.5rem;
		/* border-left: solid 3px var(--container-color); */
	}

	.header-icon {
		display: inline-flex;
		width: 1rem;
		height: 1rem;
		margin-right: 0.25rem;
	}

	.arrow {
		width: 1rem;
		height: 1rem;
		display: inline-block;
	}

	.node-inspector:not(.root) {
		.node-title {
			display: inline-flex;
			align-items: center;
			gap: 0.25rem;
			background-color: rgb(from var(--container-color) r g b / 30%);
			border-radius: 0.5rem 0.5rem 0 0;
			padding: 0.2rem;
			box-sizing: border-box;
			border-left: solid 2px var(--container-color);
			vertical-align: middle;
			cursor: pointer;
		}

		.node-header {
			display: flex;
		}

		.node-title .title-text {
			padding: 0 0.15rem;
			/* line-height: 0.5rem; */
			vertical-align: text-top;
		}

		.node-add-button-wrapper {
			border-radius: 0.5rem 0.5rem 0 0;
			padding: 0 0.25rem;
			background-color: rgb(from var(--container-color) r g b / 5%);
		}

		:global(.node-inspector-content) {
			min-height: 0.5rem;
			border-radius: 0 0.5rem 0.5rem 0.5rem;
			background-color: rgba(from var(--container-color) r g b / 5%);
			border-bottom: solid 1px rgba(from var(--container-color) r g b / 20%);
			border-left: solid 2px var(--container-color);
			padding-bottom: 0.25rem;
			box-sizing: border-box;
		}

		:global(.node-inspector-content).with-add-button {
			border-radius: 0 0 0.5rem 0.5rem;
		}
	}

	.child-row {
		display: flex;
		align-items: flex-start;
		gap: 0.25rem;
	}

	.title-text {
		margin: 0;
		padding: 0.2rem 0.5rem;
		cursor: pointer;
		color: var(--panel-accent-text-color);
		font-weight: bold;
	}

	.name-changeable .title-text {
		cursor: text;
	}

	.node-title-input {
		height: 0.9rem;
		min-width: 8rem;
		background-color: var(--bg-color);
		color: var(--text-color);
		font-size: 0.7rem;
		outline: 1px solid rgba(255, 255, 255, 0.2);
		/* border-radius: 0.25rem; */
		padding: 0 0.15rem;
		box-sizing: border-box;
		margin: 0;
	}
</style>
