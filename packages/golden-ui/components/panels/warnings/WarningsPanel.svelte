<script lang="ts">
	import type { PanelProps, PanelState } from '../../../dockview/panel-types';
	import { appState } from '../../../store/workbench.svelte';
	import type { NodeWarningRecord } from '../../../store/workbench.svelte';
	import type { NodeId } from '../../../types';
	import { copyTextToClipboard } from '../../../utils/clipboard';

	let { panelId, panelType, title, params }: PanelProps = $props();

	let panel = $state<PanelState>({
		panelId: '',
		panelType: '',
		title: '',
		params: {}
	});

	$effect(() => {
		panel = {
			panelId,
			panelType,
			title,
			params
		};
	});

	export const setPanelState = (next: PanelState): void => {
		panel = next;
	};

	let session = $derived(appState.session);
	let warnings = $derived(session?.getActiveWarnings() ?? []);

	let warningElems = $state<Record<string, HTMLElement>>({});
	let lastFlashedWarningKey = $state<string | null>(null);

	$effect(() => {
		if (typeof panel.params.sourceNodeId !== 'number') {
			lastFlashedWarningKey = null;
			return;
		}

		const warningId = typeof panel.params.warningId === 'string' ? panel.params.warningId : '';
		const warningKey = `${panel.params.sourceNodeId}:${warningId}`;
		if (warningKey === lastFlashedWarningKey) {
			return;
		}

		const warning = warnings.find(
			(w) => w.warningId === warningId && w.sourceNodeId === panel.params.sourceNodeId
		);
		if (!warning) {
			return;
		}

		flashWarning(warning);
		lastFlashedWarningKey = warningKey;
	});

	const flashWarning = (warning: { warningId: string; sourceNodeId: NodeId }): void => {
		const warningElement = warningElems[`${warning.sourceNodeId}:${warning.warningId}`];
		if (warningElement) {
			warningElement.classList.remove('highlighted');
			void warningElement.offsetWidth;
			warningElement.classList.add('highlighted');

			warningElement.scrollIntoView({ behavior: 'auto', block: 'nearest', inline: 'nearest' });

			setTimeout(() => {
				warningElement.classList.remove('highlighted');
			}, 700);
		}
	};

	const warningIdLabel = (warningId: string): string => {
		return warningId.length > 0 ? warningId : 'default';
	};

	const selectNode = (nodeId: NodeId): void => {
		session?.selectNode(nodeId);
	};

	const formatWarningForClipboard = (warning: NodeWarningRecord): string => {
		const lines = [
			'Chataigne2 warning',
			`Source node: ${warning.sourceNodeLabel} (${warning.sourceNodeId})`,
			`Target node: ${warning.targetNodeLabel} (${warning.targetNodeId})`,
			`Warning id: ${warningIdLabel(warning.warningId)}`,
			`Message: ${warning.message}`
		];
		if (warning.distance > 0) {
			lines.push(`Child warning depth: ${warning.distance}`);
		}
		if (warning.detail?.trim()) {
			lines.push('', 'Detail:', warning.detail.trim());
		}
		lines.push('', 'Raw warning:', JSON.stringify(warning, null, 2));
		return lines.join('\n');
	};

	const copyWarning = (event: MouseEvent, warning: NodeWarningRecord): void => {
		event.preventDefault();
		event.stopPropagation();
		void copyTextToClipboard(formatWarningForClipboard(warning));
	};

	interface WarningDetailLine {
		label: string | null;
		value: string;
		isSummary: boolean;
	}

	const DETAIL_LABEL_PATTERN = /^[A-Za-z][A-Za-z0-9 _-]*$/;

	const parseWarningDetail = (detail: string): WarningDetailLine[] => {
		const lines = detail.replace(/\r\n?/g, '\n').split('\n');
		const parsed: WarningDetailLine[] = [];

		for (const line of lines) {
			const raw = line.trimEnd();
			if (raw.length === 0) {
				continue;
			}

			const trimmed = raw.trimStart();
			const separator = trimmed.indexOf(':');
			if (separator > 0) {
				const label = trimmed.slice(0, separator).trim();
				if (DETAIL_LABEL_PATTERN.test(label)) {
					parsed.push({
						label,
						value: trimmed.slice(separator + 1).trim(),
						isSummary: label.toLowerCase() === 'error'
					});
					continue;
				}
			}

			parsed.push({
				label: null,
				value: trimmed,
				isSummary: false
			});
		}

		return parsed;
	};
</script>

<section class="warnings-panel">
	<header class="warnings-toolbar">
		<div class="warnings-stats">
			<strong>{warnings.length}</strong>
			<span>warnings</span>
		</div>
	</header>

	<div class="warnings-list">
		{#if warnings.length === 0}
			<div class="empty-state">No active warnings.</div>
		{:else}
			{#each warnings as warning (`${warning.sourceNodeId}:${warning.warningId}`)}
				<article
					bind:this={warningElems[`${warning.sourceNodeId}:${warning.warningId}`]}
					class="warning-item">
					<div class="warning-head">
						<button
							type="button"
							class="warning-node"
							onclick={() => selectNode(warning.sourceNodeId)}>
							{warning.sourceNodeLabel}
						</button>
						<span class="warning-id">{warningIdLabel(warning.warningId)}</span>
						<button
							type="button"
							class="copy-warning-button"
							title="Copy warning"
							aria-label={`Copy warning ${warningIdLabel(warning.warningId)} from ${warning.sourceNodeLabel}`}
							onclick={(event) => copyWarning(event, warning)}></button>
					</div>
					<p class="warning-message">{warning.message}</p>
					{#if warning.detail}
						<div class="warning-detail">
							{#each parseWarningDetail(warning.detail) as line, lineIndex (`${warning.sourceNodeId}:${warning.warningId}:${lineIndex}`)}
								<div
									class="warning-detail-line"
									class:warning-detail-line--labeled={line.label !== null}
									class:warning-detail-line--summary={line.isSummary}>
									{#if line.label !== null}
										<span class="warning-detail-label">{line.label}</span>
									{/if}
									{#if line.value.length > 0}
										<span class="warning-detail-value">{line.value}</span>
									{/if}
								</div>
							{/each}
						</div>
					{/if}
				</article>
			{/each}
		{/if}
	</div>
</section>

<style>
	.warnings-panel {
		display: flex;
		flex-direction: column;
		block-size: 100%;
		min-block-size: 0;
	}

	.warnings-toolbar {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.5rem;
		border-bottom: 0.0625rem solid
			color-mix(in srgb, var(--gc-color-panel-outline) 85%, transparent);
	}

	.warnings-stats {
		display: inline-flex;
		align-items: baseline;
		gap: 0.25rem;
		font-size: 0.75rem;
	}

	.warnings-list {
		flex: 1;
		min-block-size: 0;
		overflow: auto;
		padding: 0.5rem;
		display: flex;
		flex-direction: column;
		gap: 0.4rem;
	}

	.empty-state {
		opacity: 0.7;
		font-size: 0.78rem;
	}

	.warning-item {
		padding: 0.4rem 0.5rem;
		border: 0.0625rem solid color-mix(in srgb, var(--gc-color-panel-outline) 70%, transparent);
		border-inline-start: 0.2rem solid color-mix(in srgb, #cf9b37 75%, white 25%);
		border-radius: 0.35rem;
		background: color-mix(in srgb, var(--gc-color-panel-row) 80%, black);
		display: flex;
		flex-direction: column;
		gap: 0.2rem;
		transition: background-color 0.5s ease;
	}

	.warning-item:global(.highlighted) {
		background-color: color-mix(
			in srgb,
			var(--gc-color-panel-row) 88%,
			var(--gc-color-warning) 12%
		);
		border-color: color-mix(
			in srgb,
			var(--gc-color-warning) 58%,
			var(--gc-color-panel-outline) 42%
		);
		box-shadow: 0 0 0 0.08rem rgb(from var(--gc-color-warning) r g b / 0.16);
	}

	.warning-head {
		display: flex;
		align-items: center;
		gap: 0.4rem;
		font-size: 0.66rem;
	}

	.warning-node {
		border: 0;
		background: transparent;
		padding: 0;
		color: color-mix(in srgb, var(--gc-color-focus) 85%, white 15%);
		font-size: 0.7rem;
	}

	.warning-id {
		padding: 0.05rem 0.3rem;
		border-radius: 0.3rem;
		font-size: 0.62rem;
		text-transform: uppercase;
		background: color-mix(in srgb, var(--gc-color-panel-alt) 85%, black);
		color: color-mix(in srgb, #cf9b37 85%, white 15%);
	}

	.copy-warning-button {
		inline-size: 1rem;
		block-size: 1rem;
		margin-inline-start: auto;
		padding: 0;
		border: 0;
		border-radius: 1rem;
		background-color: transparent;
		background-image: url('../../../style/icons/clipboard.svg');
		background-position: center;
		background-repeat: no-repeat;
		background-size: 0.85rem 0.85rem;
		cursor: pointer;
		opacity: 0.68;
	}

	.copy-warning-button:hover,
	.copy-warning-button:focus-visible {
		background-color: color-mix(in srgb, var(--gc-color-warning) 12%, transparent);
		outline: none;
		opacity: 1;
	}

	.warning-message {
		margin: 0;
		font-size: 0.78rem;
		line-height: 1.3;
		user-select: text;
		word-break: break-word;
	}

	.warning-detail {
		margin: 0.1rem 0 0;
		font-size: 0.68rem;
		line-height: 1.3;
		opacity: 0.92;
		user-select: text;
		word-break: break-word;
		display: flex;
		flex-direction: column;
		gap: 0.12rem;
		padding: 0.32rem 0.4rem;
		border-radius: 0.28rem;
		border: 0.0625rem solid color-mix(in srgb, var(--gc-color-panel-outline) 65%, transparent);
		background: color-mix(in srgb, var(--gc-color-panel-alt) 75%, black 25%);
	}

	.warning-detail-line {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: 0.28rem;
	}

	.warning-detail-line--labeled .warning-detail-label {
		font-size: 0.58rem;
		font-weight: 700;
		letter-spacing: 0.03em;
		text-transform: uppercase;
		color: color-mix(in srgb, var(--gc-color-panel-outline) 40%, white 60%);
	}

	.warning-detail-value {
		white-space: pre-wrap;
		word-break: break-word;
		font-family:
			ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, 'Liberation Mono', 'Courier New',
			monospace;
	}

	.warning-detail-line--summary .warning-detail-value {
		color: color-mix(in srgb, var(--gc-color-warning) 85%, white 15%);
	}
</style>
