<script lang="ts">
	import { fade, slide } from 'svelte/transition';
	import { showPanel } from '../../store/ui-panels';
	import type { NodeWarningRecord } from '../../store/workbench.svelte';

	let { warnings = [] } = $props<{
		warnings?: NodeWarningRecord[] | null;
	}>();

	const warningPreviewText = (warning: NodeWarningRecord | undefined): string =>
		warning?.detail?.trim() || warning?.message || '';

	let safeCount = $derived(Math.max(0, Math.floor(warnings?.length ?? 0)));
	let computedTitle = $derived(
		safeCount == 1 ? warningPreviewText(warnings[0]) : `${safeCount} warnings`
	);

	let showMessage = $state(false);

	function showWarningInPanel(): void {
		const warning = warnings?.[0];
		if (!warning) {
			return;
		}

		showPanel({
			panelType: 'warnings',
			panelId: 'warnings',
			params: {
				warningId: warning.warningId,
				targetNodeId: warning.targetNodeId,
				sourceNodeId: warning.sourceNodeId
			}
		});
	}
</script>

{#if safeCount > 0}
	<div class="warning-wrapper">
		<button
			transition:fade={{ duration: 100 }}
			class="warning-badge"
			title={computedTitle}
			aria-label={computedTitle}
			tabindex="-1"
			onmouseenter={() => (showMessage = true)}
			onmouseleave={() => (showMessage = false)}
			onfocus={() => (showMessage = true)}
			// onblur={() => (showMessage = false)}
			onclick={() => showWarningInPanel()}
			onkeydown={(e) => {
				if (e.key === 'Enter' || e.key === ' ') {
					showWarningInPanel();
				}
			}}>
			<div class="warning-icon"></div>

			{#if showMessage}
				<div class="message-wrapper" transition:slide={{ duration: 150, axis: 'x' }}>
					<div class="warning-message">
						{computedTitle}
					</div>
				</div>
			{/if}
		</button>
	</div>
{/if}

<style>
	.warning-wrapper {
		position: relative;
		display: inline-flex;
		align-items: center;
		font-weight: 700;
		vertical-align: middle;
		min-width: 1.5rem;
		min-height: 1rem;
		cursor: pointer;
		color: var(--gc-color-warning);
		gap: 0.2rem;
		align-items: center;
	}

	.warning-badge {
		display: inline-flex;
		position: absolute;
		left: 0;
		transition: background-color 0.2s ease;
		padding: 0.1rem 0.3rem;
		z-index: 1000;
		font-size: 0.65rem;
		border-radius: 0.45rem;
		background-color: rgb(from var(--gc-color-warning) r g b / 0%);
		text-align: left;
	}

	.warning-wrapper:hover .warning-badge {
		background-color: hsl(from var(--gc-color-warning) h s calc(l * 0.4));
		/* z-index: 1000; */
	}

	.message-wrapper {
		z-index: 1000;
	}

	.warning-message {
		max-height: 100%;
		/* width: 15rem; */
		overflow: hidden;
		white-space: nowrap;
	}
</style>
