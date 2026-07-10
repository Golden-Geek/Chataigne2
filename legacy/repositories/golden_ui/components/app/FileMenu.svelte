<script lang="ts">
	import { executeCommand, getCommandLabel, type CommandId } from '../../store/commands.svelte';
	import ContextMenu from '../common/ContextMenu.svelte';
	import type { ContextMenuAnchor, ContextMenuItem } from '../common/context-menu';
	import {
		BROWSER_PROJECT_DIRECTORY_LABEL,
		projectFileState,
		uploadProjectFileAndLoad
	} from '../../store/project-files.svelte';
	import { hasDesktopHost } from '../../host/desktop';
	import {
		projectFileExtensionLabel,
		projectFileUploadAccept
	} from '../../store/project-file-format.svelte';

	let menuOpen = $state(false);
	let triggerElement: HTMLButtonElement | null = $state(null);
	let uploadInputElement: HTMLInputElement | null = $state(null);

	const hasDesktopDialogs = hasDesktopHost();

	const dispatchCommand = async (commandId: CommandId): Promise<void> => {
		await executeCommand(commandId, { source: 'menu' });
	};

	const openBrowserUploadPicker = (): void => {
		if (projectFileState.busy) {
			return;
		}
		uploadInputElement?.click();
	};

	const handleBrowserUploadChange = async (event: Event): Promise<void> => {
		const input = event.currentTarget;
		if (!(input instanceof HTMLInputElement)) {
			return;
		}

		const file = input.files?.[0] ?? null;
		input.value = '';
		if (!file) {
			return;
		}

		await uploadProjectFileAndLoad(file);
	};

	const commandMenuItem = (
		commandId: CommandId,
		disabled: boolean,
		overrideLabel?: string
	): ContextMenuItem => ({
		label: overrideLabel ?? getCommandLabel(commandId),
		commandId,
		disabled,
		action: () => {
			void dispatchCommand(commandId);
		}
	});

	const canOpenLast = $derived(
		Boolean(projectFileState.currentPath ?? projectFileState.lastOpenedPath)
	);
	const canBrowserSave = $derived(Boolean(projectFileState.currentPath));
	const browserUploadAccept = $derived(projectFileUploadAccept());
	const browserUploadHint = $derived(
		`Uploads ${projectFileExtensionLabel()} files to ${BROWSER_PROJECT_DIRECTORY_LABEL}`
	);

	let menuAnchor = $derived.by((): ContextMenuAnchor | null => {
		if (!triggerElement) {
			return null;
		}
		return {
			kind: 'element',
			element: triggerElement,
			placement: 'bottom-start',
			offsetRem: 0.12
		};
	});

	let menuItems = $derived.by((): ContextMenuItem[] => {
		if (hasDesktopDialogs) {
			return [
				commandMenuItem('file.new', projectFileState.busy),
				commandMenuItem('file.open', projectFileState.busy),
				commandMenuItem('file.reopenLast', projectFileState.busy || !canOpenLast),
				{ separator: true },
				commandMenuItem('file.save', projectFileState.busy),
				commandMenuItem('file.saveAs', projectFileState.busy),
				{ separator: true },
				commandMenuItem('file.preferences', false)
			];
		}

		return [
			commandMenuItem('file.new', projectFileState.busy),
			{
				label: 'Open',
				disabled: projectFileState.busy,
				submenu: [
					{
						label: 'Open Remote',
						disabled: true,
						hint: 'Browser remote picker coming later'
					},
					{
						label: 'Load From...',
						disabled: projectFileState.busy,
						hint: browserUploadHint,
						action: () => {
							openBrowserUploadPicker();
						}
					}
				]
			},
			commandMenuItem('file.reopenLast', projectFileState.busy || !canOpenLast, 'Open Last'),
			{ separator: true },
			commandMenuItem('file.save', projectFileState.busy || !canBrowserSave),
			{
				label: 'Save As',
				disabled: true,
				hint: 'Browser save dialog coming later'
			},
			{ separator: true },
			commandMenuItem('file.preferences', false)
		];
	});
</script>

<div class="gc-file-menu">
	<button
		bind:this={triggerElement}
		type="button"
		class="gc-file-menu-trigger"
		aria-label="Open File menu"
		aria-expanded={menuOpen}
		onclick={() => {
			menuOpen = !menuOpen;
		}}>
		File
	</button>

	<input
		bind:this={uploadInputElement}
		class="gc-file-menu-upload"
		type="file"
		accept={browserUploadAccept}
		onchange={handleBrowserUploadChange} />

	<ContextMenu
		bind:open={menuOpen}
		items={menuItems}
		anchor={menuAnchor}
		insideElements={[triggerElement]}
		minWidthRem={11}
		maxWidthCss="min(18rem, calc(100vw - 2rem))"
		zIndex={30} />
</div>

<style>
	.gc-file-menu {
		position: relative;
		display: flex;
		align-items: center;
	}

	.gc-file-menu-trigger {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-inline-size: 3.3rem;
		block-size: 1.45rem;
		padding: 0 0.75rem;
		border-radius: 999rem;
		border: 0.05rem solid rgb(from var(--gc-color-text) r g b / 14%);
		background: linear-gradient(
			180deg,
			rgb(from var(--gc-color-text) r g b / 10%),
			rgb(from var(--gc-color-text) r g b / 4%)
		);
		font-size: 0.68rem;
		font-weight: 700;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		color: rgb(from var(--gc-color-text) r g b / 88%);
	}

	.gc-file-menu-trigger:hover {
		border-color: rgb(from var(--gc-color-focus) r g b / 34%);
		color: var(--gc-color-text);
	}

	.gc-file-menu-upload {
		display: none;
	}
</style>
