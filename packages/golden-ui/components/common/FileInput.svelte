<script lang="ts">
	import { hasDesktopHost, openDesktopFileDialog } from '../../host/desktop';
	import type { UiFileConstraints, UiFileTypeGroup } from '../../types';
	import IconButton from './IconButton.svelte';
	import browseIcon from '../../style/icons/node/folder.svg';
	import clearIcon from '../../style/icons/panels/close.svg';

	let {
		value = '',
		enabled = true,
		readOnly = false,
		fileConstraints = null,
		label = 'File',
		oncommit
	}: {
		value?: string;
		enabled?: boolean;
		readOnly?: boolean;
		fileConstraints?: UiFileConstraints | null;
		label?: string;
		oncommit: (value: string) => unknown;
	} = $props();

	const GROUP_EXTENSIONS: Record<UiFileTypeGroup, string[]> = {
		audio: ['wav', 'wave', 'aif', 'aiff', 'flac', 'mp3', 'ogg', 'opus', 'm4a', 'aac', 'wma'],
		video: ['mp4', 'm4v', 'mov', 'avi', 'mkv', 'webm', 'mpg', 'mpeg', 'ts', 'm2ts', 'flv'],
		script: ['js', 'mjs', 'cjs']
	};

	let hasDesktopDialog = $derived(hasDesktopHost());

	let draftValue = $state('');
	let isPicking = $state(false);
	let focused = $state(false);

	const normalizeExtension = (value: string): string | null => {
		const normalized = value.trim().replace(/^\.+/, '').toLowerCase();
		return normalized.length > 0 ? normalized : null;
	};

	const collectAllowedExtensions = (): string[] => {
		const explicit = new Set<string>();
		const fromTypes = new Set<string>();

		for (const value of fileConstraints?.allowed_extensions ?? []) {
			const normalized = normalizeExtension(value);
			if (normalized) {
				explicit.add(normalized);
			}
		}

		for (const group of fileConstraints?.allowed_types ?? []) {
			for (const extension of GROUP_EXTENSIONS[group] ?? []) {
				const normalized = normalizeExtension(extension);
				if (normalized) {
					fromTypes.add(normalized);
				}
			}
		}

		if (explicit.size > 0 && fromTypes.size > 0) {
			return [...explicit].filter((value) => fromTypes.has(value));
		}
		if (explicit.size > 0) {
			return [...explicit];
		}
		if (fromTypes.size > 0) {
			return [...fromTypes];
		}
		return [];
	};

	let allowedExtensions = $derived.by(collectAllowedExtensions);
	let hasConstraintHints = $derived(
		(fileConstraints?.allowed_types.length ?? 0) > 0 ||
			(fileConstraints?.allowed_extensions.length ?? 0) > 0
	);
	let allowedExtensionsLabel = $derived.by(() => {
		if (allowedExtensions.length === 0) {
			return 'Allowed: any extension';
		}
		return `Allowed: ${allowedExtensions.map((value) => `.${value}`).join(', ')}`;
	});

	$effect(() => {
		if (!focused) draftValue = value;
	});

	const commitValue = (nextValue: string): void => {
		if (readOnly || !enabled) {
			return;
		}
		const normalized = nextValue.trim();
		if (normalized === value) {
			draftValue = normalized;
			return;
		}
		draftValue = normalized;
		void oncommit(normalized);
	};

	const browseForFile = async (): Promise<void> => {
		if (!enabled || readOnly || !hasDesktopDialog || isPicking) {
			return;
		}
		isPicking = true;
		try {
			const selected = await openDesktopFileDialog({
				allowedExtensions: allowedExtensions,
				logTag: 'file-editor'
			});
			if (selected) {
				commitValue(selected);
			}
		} finally {
			isPicking = false;
		}
	};

	const clearValue = (): void => {
		commitValue('');
	};
</script>

<div class="file-editor">
	<div class="file-input-row">
		<input
			type="text"
			class="path-input"
			aria-label={label}
			value={draftValue}
			disabled={!enabled}
			class:readonly={readOnly}
			placeholder="Select a file path"
			onfocus={() => {
				focused = true;
			}}
			onblur={() => {
				focused = false;
			}}
			onchange={(event) => {
				commitValue((event.target as HTMLInputElement).value);
			}}
			onkeydown={(event) => {
				if (event.key === 'Enter') {
					event.preventDefault();
					const target = event.target as HTMLInputElement;
					target.blur();
				}
				if (event.key === 'Escape') {
					event.preventDefault();
					draftValue = value;
					const target = event.target as HTMLInputElement;
					target.value = value;
					target.blur();
				}
			}} />

		<IconButton
			icon={browseIcon}
			label="Browse"
			title={isPicking ? 'Choosing file…' : 'Browse for a file'}
			class="browse"
			disabled={!enabled || readOnly || !hasDesktopDialog || isPicking}
			onclick={() => {
				void browseForFile();
			}} />

		<IconButton
			icon={clearIcon}
			label="Clear"
			title="Clear file"
			class="clear"
			disabled={!enabled || readOnly || draftValue.length === 0}
			onclick={clearValue} />
	</div>

	{#if hasConstraintHints}
		<div class="constraints">{allowedExtensionsLabel}</div>
	{/if}
	{#if !hasDesktopDialog}
		<div class="desktop-hint">
			Desktop file browser unavailable in this client. Enter a path manually.
		</div>
	{/if}
</div>

<style>
	.file-editor {
		display: flex;
		flex-direction: column;
		gap: 0.2rem;
		width: 100%;
		min-inline-size: 0;
	}

	.file-input-row {
		display: flex;
		align-items: center;
		gap: 0.25rem;
		width: 100%;
		min-inline-size: 0;
	}

	.path-input {
		flex: 1 1 0;
		min-inline-size: 0;
		height: 1.65rem;
		padding-inline: 0.4rem;
		font-size: 0.72rem;
		box-sizing: border-box;
	}

	.constraints,
	.desktop-hint {
		font-size: 0.62rem;
		opacity: 0.7;
		line-height: 1.2;
	}

	.desktop-hint {
		opacity: 0.55;
	}
</style>
