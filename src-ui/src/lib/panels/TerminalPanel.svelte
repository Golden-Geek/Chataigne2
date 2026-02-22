<script lang="ts">
	import { onMount } from "svelte";
	import {
		readPanelPersistedState,
		writePanelPersistedState
	} from "$lib/golden_ui/dockview/panel-persistence";
	import type { PanelProps, PanelState } from "$lib/golden_ui/dockview/panel-types";

	let { panelApi, panelId, panelType, title, params }: PanelProps = $props();

	let panel = $state<PanelState>({
		panelId: "",
		panelType: "",
		title: "",
		params: {}
	});
	let command = $state("");
	let extraLines = $state<string[]>([]);
	let publishedTitle = $state("");
	let terminalLog = $state<HTMLElement | null>(null);
	let logRestoreRaf = $state<number | null>(null);
	let logPersistRaf = $state<number | null>(null);

	const defaults = [
		"[info] UI boot sequence initialized",
		"[info] Dockview layout ready",
		"[warn] Backend connection pending"
	];

	interface TerminalPersistedState {
		logScrollTop?: number;
	}

	const normalizeLines = (value: unknown): string[] => {
		if (!Array.isArray(value)) {
			return [];
		}
		return value.filter((line): line is string => typeof line === "string");
	};

	const normalizeScrollTop = (value: unknown): number | undefined => {
		if (typeof value !== "number" || !Number.isFinite(value)) {
			return undefined;
		}
		return Math.max(0, value);
	};

	const restoreLogScroll = (params: PanelState["params"]): void => {
		if (logRestoreRaf !== null) {
			cancelAnimationFrame(logRestoreRaf);
		}

		const persistedState = readPanelPersistedState<TerminalPersistedState>(params);
		const logScrollTop = normalizeScrollTop(persistedState.logScrollTop);
		if (logScrollTop === undefined) {
			logRestoreRaf = null;
			return;
		}

		logRestoreRaf = requestAnimationFrame(() => {
			logRestoreRaf = null;
			if (!terminalLog) {
				return;
			}
			terminalLog.scrollTop = logScrollTop;
		});
	};

	const applyParams = (nextParams: Record<string, unknown>): void => {
		extraLines = normalizeLines(nextParams.lines);
		restoreLogScroll(nextParams);
	};

	$effect(() => {
		panel = {
			panelId,
			panelType,
			title,
			params
		};
		applyParams(params);
	});

	const lines = $derived([...defaults, ...extraLines]);
	const dynamicTitle = $derived(`${panel.title} ${lines.length}`);

	const setExtraLines = (nextLines: string[]): void => {
		extraLines = nextLines;
		panelApi.updateParams({ lines: nextLines });
	};

	const runCommand = (): void => {
		const trimmed = command.trim();
		if (trimmed.length === 0) {
			return;
		}

		setExtraLines([...extraLines, `> ${trimmed}`, `[ok] ${trimmed} finished`]);
		command = "";
	};

	const persistLogScroll = (): void => {
		if (logPersistRaf !== null) {
			return;
		}

		logPersistRaf = requestAnimationFrame(() => {
			logPersistRaf = null;
			if (!terminalLog) {
				return;
			}
			writePanelPersistedState(panelApi, {
				logScrollTop: terminalLog.scrollTop
			});
		});
	};

	$effect(() => {
		if (dynamicTitle === publishedTitle) {
			return;
		}

		panelApi.setTitle(dynamicTitle);
		publishedTitle = dynamicTitle;
	});

	export function setPanelState(next: PanelState): void {
		panel = next;
		applyParams(next.params);
	}

	$effect(() => {
		lines.length;
		restoreLogScroll(panel.params);
	});

	onMount(() => {
		restoreLogScroll(panel.params);
		return () => {
			if (logRestoreRaf !== null) {
				cancelAnimationFrame(logRestoreRaf);
			}
			if (logPersistRaf !== null) {
				cancelAnimationFrame(logPersistRaf);
			}
		};
	});
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

	<pre class="log" bind:this={terminalLog} onscroll={persistLogScroll} aria-label="Terminal output">
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
