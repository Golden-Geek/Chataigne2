<script lang="ts">
	import type { GraphNode, GraphSocket } from '../../types';
	import type { CanvasProps } from './contracts';
	import type { CanvasGeometry } from './geometry.svelte';
	import type { CanvasEditing } from './editing.svelte';
	import type { CanvasGestures } from './gestures.svelte';
	import type { CanvasKeyboard } from './keyboard.svelte';
	let {
		node,
		canvasProps,
		geometry,
		editing,
		gestures,
		keyboard
	}: {
		node: GraphNode;
		canvasProps: CanvasProps;
		geometry: CanvasGeometry;
		editing: CanvasEditing;
		gestures: CanvasGestures;
		keyboard: CanvasKeyboard;
	} = $props();
	let connectionDraft = $derived(gestures.connectionDraft);
	let renamingNodeId = $derived(editing.renamingNodeId);
	let selectedIds = $derived(gestures.selectedIds);
	let nodeWidth = $derived(geometry.nodeWidth);
	let nodeHeaderHeight = $derived(geometry.nodeHeaderHeight);
	let hasSocketConnection = $derived(geometry.hasSocketConnection);
	let socketExpanded = $derived(geometry.socketExpanded);
	let displaySockets = $derived(geometry.displaySockets);
	let socketDisabled = $derived(geometry.socketDisabled);
	let inputSocketContentVisible = $derived(geometry.inputSocketContentVisible);
	let socketDefaultColumnWidth = $derived(geometry.socketDefaultColumnWidth);
	let toggleSocketExpanded = $derived(geometry.toggleSocketExpanded);
	let firstCollapsedSocket = $derived(geometry.firstCollapsedSocket);
	let collapsedSocketLabel = $derived(geometry.collapsedSocketLabel);
	let collapsedSocketTitle = $derived(geometry.collapsedSocketTitle);
	let collapsedSocketConnected = $derived(geometry.collapsedSocketConnected);
	let collapsedSocketDraftTarget = $derived(geometry.collapsedSocketDraftTarget);
	let nodeHeight = $derived(geometry.nodeHeight);
	let nodeBypassPath = $derived(geometry.nodeBypassPath);
	let selectNodeBody = $derived(gestures.selectNodeBody);
	let startNodeResize = $derived(gestures.startNodeResize);
	let startConnection = $derived(gestures.startConnection);
	let finishConnection = $derived(gestures.finishConnection);
	let resetNodeSize = $derived(editing.resetNodeSize);
	let toggleNodeEnabled = $derived(editing.toggleNodeEnabled);
	let canRenameNode = $derived(editing.canRenameNode);
	let beginNodeRename = $derived(editing.beginNodeRename);
	let handleNodeTitlePointerDown = $derived(editing.handleNodeTitlePointerDown);
	let finishNodeRename = $derived(editing.finishNodeRename);
	let handleRenameKeydown = $derived(editing.handleRenameKeydown);
	let nodeContent = $derived(canvasProps.nodeContent);
	let nodeHeaderContent = $derived(canvasProps.nodeHeaderContent);
	let inputSocketContent = $derived(canvasProps.inputSocketContent);
	let outputSocketContent = $derived(canvasProps.outputSocketContent);
	let socketLabels = $derived(canvasProps.socketLabels);
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<article
	class:selected={selectedIds.has(node.id)}
	class:active={node.active}
	class:collapsed={node.collapsed === true}
	class:disabled={node.enabled === false}
	class:invalid={node.invalid}
	class:draft-target={connectionDraft?.snapNodeId === node.id}
	class="node"
	class:resizing={gestures.nodeResizeGesture !== null}
	class:dragging={gestures.nodeDragGesture !== null}
	data-socket-labels={canvasProps.socketLabels}
	style:left={`${node.position.x}rem`}
	style:top={`${node.position.y}rem`}
	style:width={`${nodeWidth(node)}rem`}
	style:height={`${nodeHeight(node)}rem`}
	style:--node-accent={node.color ?? 'var(--ga-outline)'}
	style:--node-header-height={`${nodeHeaderHeight(node)}rem`}
	data-node-id={node.id}
	onkeydown={(event) => keyboard.handleNodeKeydown(event, node.id)}
	onpointerdown={(event) => selectNodeBody(event, node)}>
	{#if node.enabled === false && node.bypassConnections && node.bypassConnections.length > 0}
		<svg
			class="node-bypass-wires"
			viewBox={`0 0 ${nodeWidth(node)} ${nodeHeight(node)}`}
			aria-hidden="true">
			{#each node.bypassConnections as bypass (`${bypass.inputSocketId}:${bypass.outputSocketId}`)}
				{@const path = nodeBypassPath(node, bypass)}
				{#if path}
					<path d={path} style:--bypass-color={bypass.color ?? node.color ?? 'var(--ga-socket)'} />
				{/if}
			{/each}
		</svg>
	{/if}
	<div class="node-header">
		{#if node.collapsed === true && firstCollapsedSocket(node, 'input')}
			{@const socket = firstCollapsedSocket(node, 'input') as GraphSocket}
			<div class="header-socket-list inputs collapsed-sockets">
				<button
					type="button"
					disabled={socketDisabled(node, socket, 'input')}
					class:incompatible={socketDisabled(node, socket, 'input')}
					class:connected={collapsedSocketConnected(node, 'input')}
					class:draft-target={collapsedSocketDraftTarget(node, 'input')}
					class="socket header-socket input collapsed-socket"
					data-node-id={node.id}
					data-socket-id={socket.id}
					title={collapsedSocketTitle(node, 'input')}
					data-socket-direction="input"
					aria-label={`${node.label}: ${socket.label} input`}
					aria-pressed={keyboard.isEndpoint(node.id, socket.id, 'input')}
					onpointerup={(event) => finishConnection(event, node.id, socket)}>
					<span class="pin" style:--socket-color={socket.color ?? 'var(--ga-socket)'}></span>
					<span>{collapsedSocketLabel(node, 'input')}</span>
				</button>
			</div>
		{:else if node.socketPlacement === 'header'}
			<div class="header-socket-list inputs">
				{#each node.inputs as socket (socket.id)}
					<button
						type="button"
						disabled={socketDisabled(node, socket, 'input')}
						class:incompatible={socketDisabled(node, socket, 'input')}
						class:connected={hasSocketConnection(node, socket.id, 'input')}
						class:draft-target={connectionDraft?.snapNodeId === node.id &&
							connectionDraft?.snapSocketId === socket.id}
						class="socket header-socket input"
						data-node-id={node.id}
						data-socket-id={socket.id}
						title={socket.valueType ?? socket.label}
						data-socket-direction="input"
						aria-label={`${node.label}: ${socket.label} input`}
						aria-pressed={keyboard.isEndpoint(node.id, socket.id, 'input')}
						onpointerup={(event) => finishConnection(event, node.id, socket)}>
						<span class="pin" style:--socket-color={socket.color ?? 'var(--ga-socket)'}></span>
						<span>{socket.label}</span>
					</button>
				{/each}
			</div>
		{:else if node.headerInputs && node.headerInputs.length > 0}
			<div class="header-socket-list inputs header-inputs-extra">
				{#each node.headerInputs as socket (socket.id)}
					<button
						type="button"
						disabled={socketDisabled(node, socket, 'input')}
						class:incompatible={socketDisabled(node, socket, 'input')}
						class:connected={hasSocketConnection(node, socket.id, 'input')}
						class:draft-target={connectionDraft?.snapNodeId === node.id &&
							connectionDraft?.snapSocketId === socket.id}
						class="socket header-socket input"
						data-node-id={node.id}
						data-socket-id={socket.id}
						title={socket.valueType ?? socket.label}
						data-socket-direction="input"
						aria-label={`${node.label}: ${socket.label} input`}
						aria-pressed={keyboard.isEndpoint(node.id, socket.id, 'input')}
						onpointerup={(event) => finishConnection(event, node.id, socket)}>
						<span class="pin" style:--socket-color={socket.color ?? 'var(--ga-socket)'}></span>
					</button>
				{/each}
			</div>
		{/if}
		{#if renamingNodeId === node.id}
			<div
				class="node-title renaming"
				title={node.description?.trim() || node.subtitle || node.label}>
				<div class="node-title-row">
					{#if node.canDisable === true}
						<button
							type="button"
							class="node-enable"
							class:enabled={node.enabled !== false}
							role="switch"
							aria-checked={node.enabled !== false}
							aria-label={`${node.enabled === false ? 'Enable' : 'Disable'} ${node.label}`}
							title={node.enabled === false ? 'Enable node' : 'Disable node'}
							onpointerdown={(event) => event.stopPropagation()}
							onclick={(event) => toggleNodeEnabled(event, node)}></button>
					{/if}
					<input
						bind:this={editing.renameInput}
						class="node-title-input"
						value={editing.renameDraft}
						aria-label={`Rename ${node.label}`}
						oninput={(event) =>
							(editing.renameDraft = (event.currentTarget as HTMLInputElement).value)}
						onkeydown={handleRenameKeydown}
						onblur={finishNodeRename}
						onpointerdown={(event) => event.stopPropagation()}
						onclick={(event) => event.stopPropagation()} />
				</div>
				{#if node.description?.trim()}
					<small>{node.description}</small>
				{:else if node.subtitle}
					<small>{node.subtitle}</small>
				{/if}
			</div>
		{:else}
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div
				class="node-title"
				title={node.description?.trim() || node.subtitle || node.label}
				onpointerdown={(event) => handleNodeTitlePointerDown(event, node)}>
				<div class="node-title-row">
					{#if node.canDisable === true}
						<button
							type="button"
							class="node-enable"
							class:enabled={node.enabled !== false}
							role="switch"
							aria-checked={node.enabled !== false}
							aria-label={`${node.enabled === false ? 'Enable' : 'Disable'} ${node.label}`}
							title={node.enabled === false ? 'Enable node' : 'Disable node'}
							onpointerdown={(event) => event.stopPropagation()}
							onclick={(event) => toggleNodeEnabled(event, node)}></button>
					{/if}
					<button
						type="button"
						class="node-title-label"
						class:editable={canRenameNode(node)}
						aria-pressed={selectedIds.has(node.id)}
						title={node.description?.trim() || node.subtitle || node.label}
						ondblclick={(event) => beginNodeRename(event, node)}
						onpointerdown={(event) => handleNodeTitlePointerDown(event, node)}
						onclick={(event) => event.stopPropagation()}>
						<strong>{node.label}</strong>
					</button>
				</div>
				{#if node.description?.trim()}
					<small>{node.description}</small>
				{:else if node.subtitle}
					<small>{node.subtitle}</small>
				{/if}
			</div>
		{/if}
		{#if canvasProps.onNodeCollapsedChange}
			<button
				type="button"
				class="node-collapse"
				aria-expanded={node.collapsed !== true}
				aria-label={`${node.collapsed ? 'Expand' : 'Collapse'} ${node.label}`}
				onpointerdown={(event) => event.stopPropagation()}
				onclick={() => keyboard.toggleNodeCollapsed(node.id)}>{node.collapsed ? '+' : '−'}</button>
		{/if}
		{#if node.warning}
			<span class="node-warning" title={node.warning} aria-label={node.warning}>!</span>
		{/if}
		{#if nodeHeaderContent && node.collapsed !== true}
			<div class="node-header-content" data-no-node-select>
				{@render nodeHeaderContent(node)}
			</div>
		{/if}
		{#if node.collapsed === true && firstCollapsedSocket(node, 'output')}
			{@const socket = firstCollapsedSocket(node, 'output') as GraphSocket}
			<div class="header-socket-list outputs collapsed-sockets">
				<button
					type="button"
					class:incompatible={socket.compatible === false}
					disabled={socketDisabled(node, socket, 'output')}
					class:connected={collapsedSocketConnected(node, 'output')}
					class="socket header-socket output collapsed-socket"
					title={collapsedSocketTitle(node, 'output')}
					data-socket-direction="output"
					aria-label={`${node.label}: ${socket.label} output`}
					aria-pressed={keyboard.isEndpoint(node.id, socket.id, 'output')}
					data-socket-id={socket.id}
					data-node-id={node.id}
					onpointerdown={(event) => startConnection(event, node.id, socket)}>
					<span>{collapsedSocketLabel(node, 'output')}</span>
					<span class="pin" style:--socket-color={socket.color ?? 'var(--ga-socket)'}></span>
				</button>
			</div>
		{:else if node.socketPlacement === 'header'}
			<div class="header-socket-list outputs">
				{#each node.outputs as socket (socket.id)}
					<button
						type="button"
						class:incompatible={socket.compatible === false}
						disabled={socketDisabled(node, socket, 'output')}
						class:connected={hasSocketConnection(node, socket.id, 'output')}
						class="socket header-socket output"
						title={socket.valueType ?? socket.label}
						data-socket-direction="output"
						aria-label={`${node.label}: ${socket.label} output`}
						aria-pressed={keyboard.isEndpoint(node.id, socket.id, 'output')}
						data-socket-id={socket.id}
						data-node-id={node.id}
						onpointerdown={(event) => startConnection(event, node.id, socket)}>
						<span>{socket.label}</span>
						<span class="pin" style:--socket-color={socket.color ?? 'var(--ga-socket)'}></span>
					</button>
				{/each}
			</div>
		{/if}
	</div>
	{#if node.collapsed !== true}
		<div class="node-body">
			{#if node.socketPlacement !== 'header'}
				<div class="socket-columns">
					<div
						class="socket-list inputs"
						style:--socket-default-width={`${socketDefaultColumnWidth(displaySockets(node, 'input', node.inputs))}rem`}>
						{#each displaySockets(node, 'input', node.inputs) as socket (socket.id)}
							<div class="socket-row input" class:component={socket.parentId}>
								<button
									type="button"
									disabled={socketDisabled(node, socket, 'input')}
									class:incompatible={socketDisabled(node, socket, 'input')}
									class:connected={hasSocketConnection(node, socket.id, 'input')}
									class:draft-target={connectionDraft?.snapNodeId === node.id &&
										connectionDraft?.snapSocketId === socket.id}
									class:component={socket.parentId}
									class="socket input"
									data-node-id={node.id}
									data-socket-id={socket.id}
									title={socket.valueType ?? socket.label}
									data-socket-direction="input"
									aria-label={`${node.label}: ${socket.label} input`}
									aria-pressed={keyboard.isEndpoint(node.id, socket.id, 'input')}
									onpointerup={(event) => finishConnection(event, node.id, socket)}>
									<span class="pin" style:--socket-color={socket.color ?? 'var(--ga-socket)'}
									></span>
									<span>{socket.label}</span>
								</button>
								{#if socket.children && socket.children.length > 0}
									<button
										type="button"
										class="socket-expander"
										aria-expanded={socketExpanded(node, socket, 'input')}
										disabled={geometry.socketHasConnectedChild(node, socket, 'input')}
										aria-label={`${socketExpanded(node, socket, 'input') ? 'Collapse' : 'Expand'} ${socket.label}`}
										title={`${socketExpanded(node, socket, 'input') ? 'Collapse' : 'Expand'} ${socket.label}`}
										onclick={() => toggleSocketExpanded(node.id, socket.id, 'input')}>
										{socketExpanded(node, socket, 'input') ? 'v' : '>'}
									</button>
								{:else}
									<span class="socket-expander-spacer"></span>
								{/if}
								{#if inputSocketContent && inputSocketContentVisible(node, socket)}
									<div
										class="socket-inline-content input"
										data-no-node-select
										role="presentation"
										onpointerdown={(event) => event.stopPropagation()}>
										{@render inputSocketContent(node, socket)}
									</div>
								{/if}
							</div>
						{/each}
					</div>
					<div class="socket-list outputs">
						{#each displaySockets(node, 'output', node.outputs) as socket (socket.id)}
							<div class="socket-row output" class:component={socket.parentId}>
								{#if socket.children && socket.children.length > 0}
									<button
										type="button"
										class="socket-expander"
										aria-expanded={socketExpanded(node, socket, 'output')}
										disabled={geometry.socketHasConnectedChild(node, socket, 'output')}
										aria-label={`${socketExpanded(node, socket, 'output') ? 'Collapse' : 'Expand'} ${socket.label}`}
										title={`${socketExpanded(node, socket, 'output') ? 'Collapse' : 'Expand'} ${socket.label}`}
										onclick={() => toggleSocketExpanded(node.id, socket.id, 'output')}>
										{socketExpanded(node, socket, 'output') ? 'v' : '<'}
									</button>
								{:else}
									<span class="socket-expander-spacer"></span>
								{/if}
								{#if outputSocketContent}
									<div
										class="socket-inline-content output"
										data-no-node-select
										role="presentation"
										onpointerdown={(event) => event.stopPropagation()}>
										{@render outputSocketContent(node, socket)}
									</div>
								{/if}
								<button
									type="button"
									disabled={socketDisabled(node, socket, 'output')}
									class:incompatible={socketDisabled(node, socket, 'output')}
									class:connected={hasSocketConnection(node, socket.id, 'output')}
									class:component={socket.parentId}
									class="socket output"
									title={socket.valueType ?? socket.label}
									data-socket-direction="output"
									aria-label={`${node.label}: ${socket.label} output`}
									aria-pressed={keyboard.isEndpoint(node.id, socket.id, 'output')}
									data-socket-id={socket.id}
									data-node-id={node.id}
									onpointerdown={(event) => startConnection(event, node.id, socket)}>
									<span>{socket.label}</span>
									<span class="pin" style:--socket-color={socket.color ?? 'var(--ga-socket)'}
									></span>
								</button>
							</div>
						{/each}
					</div>
				</div>
			{/if}
			{#if nodeContent}
				<div class="node-content" data-no-node-select>
					{@render nodeContent(node)}
				</div>
			{/if}
		</div>
	{/if}
	{#if node.resizable && node.collapsed !== true}
		<button
			type="button"
			class="resize-handle"
			aria-label={`Resize ${node.label}`}
			title={`Resize ${node.label}; double-click for automatic size`}
			onpointerdown={(event) => startNodeResize(event, node)}
			ondblclick={(event) => resetNodeSize(event, node)}></button>
	{/if}
</article>

<style>
	.node-collapse {
		flex: 0 0 auto;
		align-self: center;
		border: 0;
		background: transparent;
		color: inherit;
		cursor: pointer;
		font: inherit;
	}
	.node {
		position: absolute;
		box-sizing: border-box;
		border: solid 0.08rem color-mix(in srgb, var(--node-accent) 44%, transparent);
		border-radius: 0.55rem;
		background: var(--ga-node);
		box-shadow: 0 0.55rem 1.3rem rgb(0 0 0 / 0.28);
		color: var(--gc-color-text, #e8edf6);
		filter: saturate(0.86);
		overflow: hidden;
		transition:
			width 0.18s cubic-bezier(0.2, 0, 0.13, 1),
			height 0.18s cubic-bezier(0.2, 0, 0.13, 1),
			border-color 0.46s ease,
			box-shadow 0.46s ease,
			filter 0.46s ease;
	}

	.node.resizing {
		transition:
			border-color 0.46s ease,
			box-shadow 0.46s ease;
	}

	.node.selected {
		border-color: var(--ga-selection);
	}

	.node.active {
		border-color: color-mix(in srgb, var(--node-accent) 88%, white 12%);
		filter: saturate(1);
		box-shadow:
			inset 0 0 0 0.02rem color-mix(in srgb, var(--node-accent) 52%, transparent),
			0 0.55rem 1.3rem rgb(0 0 0 / 0.28),
			0 0 0.22rem color-mix(in srgb, var(--node-accent) 20%, transparent);
		transition:
			width 0.18s cubic-bezier(0.2, 0, 0.13, 1),
			height 0.18s cubic-bezier(0.2, 0, 0.13, 1),
			border-color 0.06s ease-out,
			box-shadow 0.06s ease-out,
			filter 0.06s ease-out;
	}

	.node.disabled {
		border-style: dashed;
		color: color-mix(in srgb, var(--gc-color-text, #e8edf6) 64%, transparent);
	}

	.node.selected.active {
		border-color: var(--ga-selection);
		box-shadow:
			inset 0 0 0 0.02rem color-mix(in srgb, var(--gc-color-selection) 58%, transparent),
			0 0.55rem 1.3rem rgb(0 0 0 / 0.28),
			0 0 0.5rem color-mix(in srgb, var(--gc-color-selection) 52%, transparent);
	}

	.node.invalid {
		border-color: var(--ga-error);
	}

	.node.draft-target {
		border-color: var(--ga-selection);
		box-shadow: 0 0 0.8rem color-mix(in srgb, var(--gc-color-selection) 60%, transparent);
	}

	.socket.draft-target .pin {
		background: var(--ga-selection);
		box-shadow: 0 0 0.4rem var(--ga-selection);
	}

	.node-bypass-wires {
		position: absolute;
		z-index: 1;
		inset: 0;
		inline-size: 100%;
		block-size: 100%;
		pointer-events: none;
	}

	.node-bypass-wires path {
		fill: none;
		stroke: var(--bypass-color, var(--ga-socket));
		stroke-width: 0.11rem;
		stroke-dasharray: 0.28rem 0.22rem;
		opacity: 0.78;
		vector-effect: non-scaling-stroke;
	}

	.node-header {
		position: relative;
		z-index: 2;
		display: flex;
		align-items: stretch;
		inline-size: 100%;
		block-size: var(--node-header-height, 1.8rem);
		min-block-size: var(--node-header-height, 1.8rem);
		box-sizing: border-box;
		border-block-end: solid 0.06rem color-mix(in srgb, var(--node-accent) 62%, transparent);
		border-radius: 0.48rem 0.48rem 0 0;
		background: color-mix(in srgb, var(--node-accent) 28%, var(--ga-node));
		color: inherit;
		overflow: hidden;
	}

	.node-body {
		position: relative;
		z-index: 2;
	}

	.node.collapsed .node-header {
		border-block-end: 0;
		border-radius: 0.48rem;
	}

	.node-enable {
		position: relative;
		flex: 0 0 0.85rem;
		align-self: center;
		inline-size: 0.85rem;
		block-size: 1rem;
		padding: 0;
		border: 0;
		background: transparent;
		cursor: pointer;
	}

	.node-enable::before {
		content: '';
		position: absolute;
		inset: 50% auto auto 50%;
		box-sizing: border-box;
		inline-size: 0.54rem;
		block-size: 0.54rem;
		border: solid 0.06rem color-mix(in srgb, var(--gc-color-text, #e8edf6) 56%, transparent);
		border-radius: 999rem;
		background: color-mix(in srgb, var(--ga-bg) 68%, transparent);
		transform: translate(-50%, -50%);
		transition:
			background 0.14s ease,
			border-color 0.14s ease,
			box-shadow 0.14s ease;
	}

	.node-enable.enabled::before {
		border-color: color-mix(in srgb, var(--node-accent) 78%, white 12%);
		background: color-mix(in srgb, var(--node-accent) 82%, white 6%);
		box-shadow: 0 0 0.28rem color-mix(in srgb, var(--node-accent) 70%, transparent);
	}

	.node-title {
		display: flex;
		flex: 1 1 auto;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		min-inline-size: 0;
		padding: 0.15rem 0.35rem;
		box-sizing: border-box;
		border: 0;
		background: transparent;
		color: inherit;
		font: inherit;
		text-align: center;
		cursor: default;
	}

	.node.dragging .node-title {
		cursor: grabbing;
	}

	.node-title.renaming {
		cursor: text;
	}

	.node-title-row {
		display: flex;
		align-items: center;
		justify-content: center;
		inline-size: 100%;
		min-inline-size: 0;
		gap: 0.18rem;
	}

	.node-title-label {
		display: block;
		flex: 0 1 auto;
		inline-size: auto;
		min-inline-size: 0;
		max-inline-size: 100%;
		padding: 0;
		border: 0;
		background: transparent;
		color: inherit;
		font: inherit;
		font-weight: 700;
		text-align: center;
		cursor: default;
	}

	.node .node-title-label strong {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.node-title-input {
		flex: 1 1 auto;
		inline-size: 100%;
		min-inline-size: 0;
		padding: 0.12rem 0.2rem;
		border: solid 0.06rem color-mix(in srgb, var(--ga-selection) 76%, transparent);
		border-radius: 0.25rem;
		background: color-mix(in srgb, var(--ga-bg) 86%, transparent);
		color: inherit;
		font: inherit;
		font-size: 0.76rem;
		font-weight: 700;
		outline: none;
	}

	.node-warning {
		display: inline-grid;
		flex: 0 0 1.05rem;
		align-self: center;
		place-items: center;
		inline-size: 1.05rem;
		block-size: 1.05rem;
		margin-inline-end: 0.25rem;
		border: 0.06rem solid color-mix(in srgb, var(--ga-error) 74%, transparent);
		border-radius: 999rem;
		background: color-mix(in srgb, var(--ga-error) 18%, transparent);
		color: var(--ga-error);
		font-size: 0.72rem;
		font-weight: 800;
		line-height: 1;
	}

	/* .node.active .node-header {
		background: color-mix(in srgb, var(--ga-active) 22%, var(--ga-node));
	} */

	.node strong,
	.node small {
		display: block;
		overflow-wrap: anywhere;
		white-space: normal;
	}

	.node strong {
		font-size: 0.76rem;
		line-height: 1.08;
	}

	.node small {
		font-size: 0.64rem;
		line-height: 1.1;
		opacity: 0.62;
	}

	.header-socket-list {
		display: flex;
		flex: 0 0 auto;
		align-items: stretch;
		min-inline-size: 0;
	}

	.header-socket {
		block-size: 100%;
		min-block-size: 1.72rem;
		padding-inline: 0.4rem;
		font-size: 0.64rem;
	}

	.node-header-content {
		display: flex;
		flex: 0 1 auto;
		align-items: center;
		min-inline-size: 0;
		max-inline-size: 45%;
	}

	.collapsed-socket {
		min-inline-size: 1.75rem;
		justify-content: center;
	}

	.node-body {
		display: flex;
		flex-direction: column;
		box-sizing: border-box;
		block-size: calc(100% - var(--node-header-height, 1.8rem));
		min-block-size: 1.75rem;
	}

	.socket-columns {
		display: grid;
		grid-template-columns: minmax(0, 1fr) max-content;
		column-gap: 0.2rem;
		align-content: start;
		box-sizing: border-box;
		flex: 0 0 auto;
		min-block-size: 1.35rem;
		padding-block: 0.2rem;
	}

	.socket-list {
		display: flex;
		flex-direction: column;
		gap: 0;
		min-inline-size: 0;
	}

	.socket-list.inputs {
		display: grid;
		grid-template-columns: minmax(0, max-content) 0.85rem minmax(
				0,
				var(--socket-default-width, 0rem)
			);
		align-items: start;
		justify-self: start;
		max-inline-size: 100%;
		overflow: hidden;
	}

	.socket-list.outputs {
		min-inline-size: max-content;
	}

	.socket-row {
		display: flex;
		align-items: center;
		block-size: 1.35rem;
		min-inline-size: 0;
	}

	.socket-row.input {
		display: grid;
		grid-column: 1 / -1;
		grid-template-columns: subgrid;
		align-items: center;
		inline-size: 100%;
		max-inline-size: 100%;
	}

	.socket-row.output {
		justify-content: flex-end;
	}

	.socket-row.component {
		opacity: 0.9;
	}

	.socket-row.input.component {
		padding-inline-end: 0.45rem;
	}

	.socket-row.output.component {
		padding-inline-start: 0.45rem;
	}

	.socket-expander,
	.socket-expander-spacer {
		flex: 0 0 0.85rem;
		inline-size: 0.85rem;
		block-size: 100%;
	}

	.socket-expander {
		display: inline-grid;
		place-items: center;
		padding: 0;
		border: 0;
		background: transparent;
		color: color-mix(in srgb, var(--gc-color-text, #e8edf6) 58%, transparent);
		font: inherit;
		font-size: 0.58rem;
		cursor: pointer;
	}

	.socket-expander:hover {
		color: var(--gc-color-text, #e8edf6);
	}

	.socket-inline-content {
		display: flex;
		align-items: center;
		justify-content: stretch;
		min-inline-size: 0;
		max-inline-size: 100%;
		overflow: hidden;
	}

	.socket-inline-content.input {
		inline-size: 100%;
	}

	.socket-inline-content.output {
		flex: 0 1 auto;
		justify-content: flex-end;
		inline-size: auto;
		max-inline-size: 7rem;
	}

	.socket {
		position: relative;
		display: flex;
		align-items: center;
		gap: 0.25rem;
		block-size: 1.35rem;
		min-inline-size: 0;
		padding: 0 0.4rem;
		border: 0;
		background: transparent;
		color: inherit;
		font: inherit;
		font-size: 0.68rem;
		cursor: crosshair;
	}

	.socket-row .socket {
		flex: 0 0 auto;
		block-size: 100%;
	}

	/* In grid/subgrid context (input rows), override flex-dependent sizing */
	.socket-row.input .socket.input {
		block-size: 1.35rem;
		inline-size: 100%;
		max-inline-size: 100%;
	}

	.socket-columns .socket.input {
		justify-content: flex-start;
		/* padding-inline-start: 0.12rem; */
		padding-inline-end: 0.35rem;
	}

	.socket-columns .socket.output {
		justify-content: flex-end;
		padding-inline-start: 0.35rem;
		/* padding-inline-end: 0.12rem; */
	}

	.socket span:not(.pin) {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		transition: opacity 0.15s ease;
	}

	.node[data-socket-labels='never'] .socket span:not(.pin) {
		opacity: 0;
	}

	.node[data-socket-labels='hover'] .socket span:not(.pin) {
		opacity: 0;
	}

	.node[data-socket-labels='hover'] .socket:hover span:not(.pin) {
		opacity: 1;
	}

	.socket.incompatible {
		opacity: 0.34;
		cursor: not-allowed;
	}

	.socket.component {
		font-size: 0.62rem;
	}

	.pin {
		position: relative;
		z-index: 0;
		box-sizing: border-box;
		inline-size: 0.5rem;
		block-size: 0.5rem;
		border: solid 0.05rem color-mix(in srgb, var(--socket-color) 70%, transparent);
		border-radius: 50%;
		background: var(--ga-node);
		box-shadow: 0 0 0 0.08rem rgb(0 0 0 / 0.32);
	}

	.socket:hover .pin,
	.socket.connected .pin {
		background: var(--socket-color);
	}

	.socket.connected .pin::after {
		content: '';
		position: absolute;
		top: 50%;
		transform: translateY(-50%);
		width: 0.75rem;
		height: 0.16rem;
		background: color-mix(in srgb, var(--socket-color) 72%, transparent);
		z-index: -1;
		pointer-events: none;
	}

	.socket.input.connected .pin::after {
		right: 50%;
	}

	.socket.output.connected .pin::after {
		left: 50%;
	}

	.node-content {
		flex: 1 1 auto;
		min-block-size: 0;
		/* padding: 0.18rem 0.35rem 0.35rem; */
		/* overflow: auto; */
		user-select: text;
	}

	.resize-handle {
		position: absolute;
		z-index: 3;
		inset-inline-end: 0.12rem;
		inset-block-end: 0.12rem;
		inline-size: 1rem;
		block-size: 1rem;
		padding: 0;
		border: 0;
		background: linear-gradient(
			135deg,
			transparent 0 52%,
			color-mix(in srgb, var(--ga-outline) 72%, transparent) 52% 60%,
			transparent 60% 68%,
			color-mix(in srgb, var(--ga-outline) 72%, transparent) 68% 76%,
			transparent 76%
		);
		cursor: nwse-resize;
	}

	.resize-handle:hover {
		background: linear-gradient(
			135deg,
			transparent 0 52%,
			color-mix(in srgb, var(--ga-selection) 88%, transparent) 52% 60%,
			transparent 60% 68%,
			color-mix(in srgb, var(--ga-selection) 88%, transparent) 68% 76%,
			transparent 76%
		);
	}
</style>
