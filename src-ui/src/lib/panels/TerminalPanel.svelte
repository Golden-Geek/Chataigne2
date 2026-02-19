<script lang="ts">
	import type { DockPanelProps, DockPanelState } from "$lib/dockview/panel-types";

	const initialProps: DockPanelProps = $props();
	const api = initialProps.api;

	let panel = $state<DockPanelState>({
		panelId: initialProps.panelId,
		title: initialProps.title,
		params: initialProps.params
	});
	let command = $state("");
	let lines = $state<string[]>([]);
	let publishedTitle = $state("");

	const defaults = [
		"[info] UI boot sequence initialized",
		"[info] Dockview layout ready",
		"[warn] Backend connection pending"
	];

	const applyParams = (nextParams: Record<string, unknown>): void => {
		const extraLines = (nextParams.lines as string[] | undefined) ?? [];
		lines = [...defaults, ...extraLines];
	};

	applyParams(initialProps.params);

	const dynamicTitle = $derived(`${panel.title} ${lines.length}`);

	const runCommand = (): void => {
		const trimmed = command.trim();
		if (trimmed.length === 0) {
			return;
		}

		lines = [...lines, `> ${trimmed}`, `[ok] ${trimmed} finished`];
		command = "";
	};

	$effect(() => {
		if (dynamicTitle === publishedTitle) {
			return;
		}

		api.setTitle(dynamicTitle);
		publishedTitle = dynamicTitle;
	});

	export function setDockPanelState(next: DockPanelState): void {
		panel = next;
		applyParams(next.params);
	}
</script>

<section class="panel terminal">
	<header class="panel-header">
		<h2>{dynamicTitle}</h2>
		<p>{panel.panelId}</p>
	</header>

	<div class="terminal-actions">
		<input
			bind:value={command}
			type="text"
			placeholder="Type command"
			onkeydown={(event) => {
				if (event.key === "Enter") {
					runCommand();
				}
			}}
		/>
		<button type="button" onclick={runCommand}>Run</button>
	</div>

	<pre class="log" aria-label="Terminal output">
{#each lines as line}
{line}
{/each}
	</pre>
</section>

<style>
	.panel {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
		inline-size: 100%;
		block-size: 100%;
		padding: 1rem;
		box-sizing: border-box;

	}

	.panel-header h2 {
		margin: 0;
		font-size: 0.95rem;
		line-height: 1.2;
	}

	.panel-header p {
		margin: 0.3rem 0 0;
		font-size: 0.7rem;
		opacity: 0.72;
	}

	.terminal-actions {
		display: flex;
		gap: 0.5rem;
	}

	.terminal-actions input {
		flex: 1;
		padding: 0.45rem 0.6rem;
		border: 0.0625rem solid var(--gc-color-panel-outline);
		border-radius: 0.35rem;
		outline: none;
		color: inherit;
		background: color-mix(in srgb, var(--gc-color-terminal) 88%, black);
	}

	.terminal-actions input:focus {
		border-color: var(--gc-color-focus);
	}

	.terminal-actions button {
		padding: 0.45rem 0.7rem;
		border: 0.0625rem solid var(--gc-color-panel-outline);
		border-radius: 0.35rem;
		background: var(--gc-color-panel-row);
		color: inherit;
		cursor: pointer;
	}

	.log {
		flex: 1;
		min-block-size: 0;
		margin: 0;
		padding: 0.75rem;
		border-radius: 0.4rem;
		font-size: 0.76rem;
		line-height: 1.45;
		white-space: pre-wrap;
		overflow: auto;
		background: var(--gc-color-terminal);
		color: var(--gc-color-terminal-text);
		border: 0.0625rem solid color-mix(in srgb, var(--gc-color-focus) 45%, transparent);
	}
</style>
