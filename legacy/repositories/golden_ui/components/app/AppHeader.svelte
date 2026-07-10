<script lang="ts">
	import type { WorkbenchSession } from '../../store/workbench.svelte';
	import { hasDesktopHost, invokeDesktopCommand } from '../../host/desktop';
	import { onMount } from 'svelte';
	import { clearPersistedUiState } from '../../store/ui-persistence';
	import { platform } from '../../store/platform.svelte';
	import { projectFileDisplayName, projectFileState } from '../../store/project-files.svelte';
	import { requestWindowExit } from '../../store/window-exit.svelte';
	import FileMenu from './FileMenu.svelte';
	import NetworkStatus from './NetworkStatus.svelte';

	let isWindowMaximized = $state(false);
	let hasTauriWindowApi = $state(false);
	let useManualHeaderDrag = $derived(hasTauriWindowApi && platform.isWindows);
	let useNativeHeaderDrag = $derived(hasTauriWindowApi && !useManualHeaderDrag);

	const DRAG_EXCLUDED_SELECTOR = 'button, input, textarea, select, a, [data-no-drag]';

	const { session } = $props<{
		session?: WorkbenchSession | null;
	}>();

	let projectTitle = $derived.by((): string => {
		const fileName = projectFileDisplayName(projectFileState.currentPath);
		return session.currentHistoryStateId !== projectFileState.savedHistoryStateId
			? `${fileName}*`
			: fileName;
	});
	let engineRateClass = $derived.by((): string => {
		const hz = session.engineHz;
		if (hz === null) {
			return 'unknown';
		}
		const lowFrequency = Math.max(1, session.engineLowFrequencyHz);
		if (hz < lowFrequency / 2) {
			return 'low';
		}
		if (hz <= lowFrequency) {
			return 'mid';
		}
		return 'high';
	});
	let engineRateLabel = $derived.by((): string => {
		const hz = session.engineHz;
		return hz === null ? '-- Hz' : `${Math.round(hz)} Hz`;
	});

	const refreshMaximizeState = async (): Promise<void> => {
		const maximized = await invokeDesktopCommand(
			'window_is_maximized',
			undefined,
			'window-controls'
		);
		isWindowMaximized = Boolean(maximized);
	};

	const minimizeWindow = async (): Promise<void> => {
		const result = await invokeDesktopCommand('window_minimize', undefined, 'window-controls');
		if (result === undefined) {
			console.error('[window-controls] Tauri window API unavailable (minimize).');
		}
	};

	const toggleWindowMaximize = async (): Promise<void> => {
		const result = await invokeDesktopCommand(
			'window_toggle_maximize',
			undefined,
			'window-controls'
		);
		if (result === undefined) {
			console.error('[window-controls] Tauri window API unavailable (toggle maximize).');
			return;
		}
		await refreshMaximizeState();
	};

	const closeWindow = async (): Promise<void> => {
		await requestWindowExit();
	};

	const startDrag = async (): Promise<void> => {
		const result = await invokeDesktopCommand('start_drag', undefined, 'window-controls');
		if (result === undefined) {
			console.error('[window-controls] Tauri window API unavailable (start drag)');
		}
	};

	const isHeaderDragExcludedTarget = (target: EventTarget | null): boolean => {
		return target instanceof Element && target.closest(DRAG_EXCLUDED_SELECTOR) !== null;
	};

	let lastHeaderClickTime = 0;

	const handleHeaderMouseDown = (event: MouseEvent): void => {
		if (event.button !== 0 || isHeaderDragExcludedTarget(event.target)) {
			return;
		}

		const now = Date.now();
		const isDoubleClick = now - lastHeaderClickTime < 400;
		lastHeaderClickTime = now;

		if (isDoubleClick) {
			lastHeaderClickTime = 0;
			event.preventDefault();
			void toggleWindowMaximize();
			return;
		}

		const target = event.target as Element;
		const isSpacer = target.classList?.contains('spacer') ?? false;

		if (!useManualHeaderDrag && !isSpacer) {
			return;
		}

		event.preventDefault();
		void startDrag();
	};

	const bindHeaderInteraction = (element: HTMLDivElement): { destroy: () => void } => {
		const onMouseDown = (event: MouseEvent): void => {
			handleHeaderMouseDown(event);
		};

		element.addEventListener('mousedown', onMouseDown);

		return {
			destroy: (): void => {
				element.removeEventListener('mousedown', onMouseDown);
			}
		};
	};

	const clearUiStorage = (): void => {
		clearPersistedUiState();
		if (typeof window !== 'undefined') {
			window.location.reload();
		}
	};

	onMount(() => {
		hasTauriWindowApi = hasDesktopHost();
		if (!hasTauriWindowApi) {
			return;
		}

		void refreshMaximizeState();

		const onResize = () => {
			void refreshMaximizeState();
		};
		window.addEventListener('resize', onResize);

		return () => {
			window.removeEventListener('resize', onResize);
		};
	});
</script>

<div
	class="gc-header {hasTauriWindowApi ? 'tauri' : ''} {useManualHeaderDrag ? 'manual-drag' : ''}"
	role="banner"
	use:bindHeaderInteraction>
	<div class="header-start">
		<FileMenu />
		<div class="app-title">
			<span class="project-title">{projectTitle}</span>
			<span class="app-title-app">Chataigne 2.0.0</span>
			<span class="engine-rate {engineRateClass}" title="Engine framerate">
				<span class="engine-rate-dot" aria-hidden="true"></span>
				<span>{engineRateLabel}</span>
			</span>
		</div>
	</div>
	<div
		class="spacer"
		data-tauri-drag-region={useNativeHeaderDrag ? '' : undefined}
		role="button"
		tabindex="-1"
		onkeydown={() => {}}>
	</div>
	<div class="controls">
		<div class="history-actions">
			<button
				type="button"
				class="history-button clear-ui"
				title="Clear persisted UI state and reload"
				onclick={clearUiStorage}>
				Clear UI
			</button>
			<button
				type="button"
				class="history-button undo"
				title="Ctrl/Cmd+Z"
				disabled={session.historyBusy || !session.canUndo}
				onclick={() => void session.undo()}>
			</button>
			<button
				type="button"
				class="history-button redo"
				title="Ctrl/Cmd+Shift+Z or Ctrl+Y"
				disabled={session.historyBusy || !session.canRedo}
				onclick={() => void session.redo()}>
			</button>
		</div>
	</div>
	<div
		class="spacer"
		data-tauri-drag-region={useNativeHeaderDrag ? '' : undefined}
		role="button"
		tabindex="-1"
		onkeydown={() => {}}>
	</div>
	<NetworkStatus httpBaseUrl={session.httpBaseUrl} />
	{#if hasTauriWindowApi}
		<div class="app-buttons">
			<button
				type="button"
				class="minimize-app"
				aria-label="Minimize app"
				onclick={() => minimizeWindow()}>➖</button>
			<button
				type="button"
				class="maximize-app"
				aria-label="Maximize app"
				onclick={() => toggleWindowMaximize()}>🟩</button>
			<button type="button" class="close-app" aria-label="Close app" onclick={() => closeWindow()}
				>🔴</button>
		</div>
	{:else}
		<div class="footer-status">
			<span class="status-badge {session.status}">
				<span class="status-dot" aria-hidden="true"></span>
				<span class="status-label">{session.status}</span>
			</span>
		</div>
	{/if}
</div>

<style>
	.header-start {
		display: flex;
		align-items: center;
		gap: 0.7rem;
		min-width: 0;
	}

	.app-title {
		display: inline-flex;
		align-items: baseline;
		gap: 0.55rem;
		min-width: 0;
		white-space: nowrap;
	}

	.project-title {
		max-width: min(34vw, 28rem);
		overflow: hidden;
		text-overflow: ellipsis;
		font-size: 0.82rem;
		font-weight: 700;
		letter-spacing: 0.02em;
		color: rgb(from var(--gc-color-text) r g b / 92%);
	}

	.app-title-app {
		font-size: 0.68rem;
		font-weight: 700;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		color: rgb(from var(--gc-color-text) r g b / 58%);
	}

	.engine-rate {
		display: inline-flex;
		align-items: center;
		gap: 0.35rem;
		min-width: 3.8rem;
		padding: 0.15rem 0.42rem;
		border: 0.05rem solid currentColor;
		border-radius: 0.35rem;
		font-size: 0.68rem;
		font-weight: 700;
		line-height: 1;
		color: rgb(from var(--gc-color-text) r g b / 45%);
		background-color: rgb(from var(--gc-color-text) r g b / 7%);
		-webkit-app-region: no-drag;
	}

	.engine-rate.low {
		color: var(--gc-color-error);
		background-color: rgb(from var(--gc-color-error) r g b / 12%);
	}

	.engine-rate.mid {
		color: var(--gc-color-warning);
		background-color: rgb(from var(--gc-color-warning) r g b / 12%);
	}

	.engine-rate.high {
		color: var(--gc-color-success);
		background-color: rgb(from var(--gc-color-success) r g b / 12%);
	}

	.engine-rate-dot {
		width: 0.45rem;
		height: 0.45rem;
		border-radius: 50%;
		background-color: currentColor;
		box-shadow: 0 0 0.35rem currentColor;
	}

	.controls {
		display: flex;
		align-items: center;
	}

	.history-actions {
		display: flex;
		gap: 0.25rem;
	}

	.history-actions button {
		transition:
			filter 0.2s ease,
			opacity 0.2s ease;
	}

	.history-actions button:hover:not(:disabled) {
		filter: brightness(1.2);
	}

	.clear-ui {
		background-color: rgb(200 200 200 / 20%);
		border: 1px solid rgb(from var(--gc-color-text) r g b / 20%);
		border-radius: 0.25rem;
		padding: 0.25rem 0.5rem;
		transition: opacity 0.2s;
		font-size: 0.6rem;
	}

	.undo,
	.redo {
		background-repeat: no-repeat;
		background-position: center;
		background-size: contain;
		box-sizing: border-box;
		width: 1.5rem;
		height: 1.5rem;
	}

	.undo {
		background-image: url('../../style/icons/undo.svg');
	}

	.redo {
		background-image: url('../../style/icons/redo.svg');
	}

	.undo:disabled,
	.redo:disabled {
		opacity: 0.5;
		cursor: default;
		pointer-events: none;
		touch-action: none;
	}

	.app-buttons {
		width: 20%;
		text-align: right;
	}

	.footer-status {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		align-self: center;
	}

	.status-badge {
		display: inline-flex;
		align-items: center;
		gap: 0.45rem;
		padding: 0.3rem 0.65rem;
		border-radius: 999rem;
		font-size: 0.68rem;
		font-weight: 600;
		letter-spacing: 0.04em;
		background-color: rgb(from var(--gc-color-text) r g b / 8%);
		border: 0.05rem solid rgb(from var(--gc-color-text) r g b / 14%);
	}

	.status-badge.disconnected {
		color: var(--gc-color-error);
		background-color: rgb(from var(--gc-color-error) r g b / 12%);
		border-color: rgb(from var(--gc-color-error) r g b / 24%);
	}

	.status-badge.connecting {
		color: var(--gc-color-warning);
		background-color: rgb(from var(--gc-color-warning) r g b / 12%);
		border-color: rgb(from var(--gc-color-warning) r g b / 24%);
	}

	.status-badge.connected {
		color: var(--gc-color-success);
		background-color: rgb(from var(--gc-color-success) r g b / 12%);
		border-color: rgb(from var(--gc-color-success) r g b / 24%);
	}

	.status-dot {
		width: 0.55rem;
		height: 0.55rem;
		border-radius: 50%;
		background-color: currentColor;
		box-shadow: 0 0 0.45rem currentColor;
	}

	.status-label {
		line-height: 1;
	}

	.gc-header.manual-drag {
		-webkit-app-region: no-drag;
	}
</style>
