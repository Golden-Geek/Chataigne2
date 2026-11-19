<script lang="ts" module>
	export interface FileDropDestination {
		bounds: Element;
		label: string;
		action: 'add' | 'replace';
		icon?: string;
		drop: (paths: string[]) => Promise<void>;
	}
</script>

<script lang="ts">
	import { hasDesktopFileDrop, readDroppedFilePaths } from '../../host/file-drop';
	import addIcon from '../../style/icons/node/add.svg';
	import replaceIcon from '../../style/icons/reset.svg';

	let {
		area,
		resolve
	}: {
		area: HTMLElement | undefined;
		resolve: (element: Element, event: DragEvent) => FileDropDestination | null;
	} = $props();

	let active = $state(false);
	let error = $state('');
	let frame = $state.raw<{
		icon: string;
		label: string;
		left: number;
		top: number;
		width: number;
		height: number;
	} | null>(null);

	function isFileDrag(event: DragEvent): boolean {
		return event.dataTransfer?.types.includes('Files') ?? false;
	}

	$effect(() => {
		const root = area;
		if (!root || !hasDesktopFileDrop()) return;
		const owner = new AbortController();
		let clearError: ReturnType<typeof setTimeout> | undefined;
		const target = (event: DragEvent) =>
			event.target instanceof Element ? resolve(event.target, event) : null;
		const move = (event: DragEvent) => {
			if (!isFileDrag(event)) return;
			event.preventDefault();
			const destination = target(event);
			if (event.dataTransfer) event.dataTransfer.dropEffect = destination ? 'copy' : 'none';
			active = destination !== null;
			if (!destination) return;
			const box = destination.bounds.getBoundingClientRect();
			const parent = root.getBoundingClientRect();
			frame = {
				icon: destination.icon ?? (destination.action === 'add' ? addIcon : replaceIcon),
				label: destination.label,
				left: box.left - parent.left,
				top: box.top - parent.top,
				width: box.width,
				height: box.height
			};
		};
		const leave = (event: DragEvent) => {
			if (!(event.relatedTarget instanceof Node) || !root.contains(event.relatedTarget)) {
				active = false;
			}
		};
		const drop = async (event: DragEvent) => {
			if (!isFileDrag(event)) return;
			event.preventDefault();
			event.stopPropagation();
			active = false;
			const destination = target(event);
			const files = event.dataTransfer?.files;
			if (!destination || !files?.length) return;
			clearTimeout(clearError);
			error = '';
			try {
				const paths = await readDroppedFilePaths(files, owner.signal);
				owner.signal.throwIfAborted();
				await destination.drop(paths);
			} catch (cause) {
				if (owner.signal.aborted) return;
				error = cause instanceof Error ? cause.message : String(cause);
				clearError = setTimeout(() => (error = ''), 5000);
			}
		};
		root.addEventListener('dragover', move, { capture: true, signal: owner.signal });
		root.addEventListener('dragleave', leave, { capture: true, signal: owner.signal });
		root.addEventListener('drop', drop, { capture: true, signal: owner.signal });
		return () => {
			owner.abort();
			clearTimeout(clearError);
		};
	});
</script>

{#if frame}
	<div
		class="file-drop-feedback"
		class:active
		aria-hidden="true"
		style:left="{frame.left}px"
		style:top="{frame.top}px"
		style:width="{frame.width}px"
		style:height="{frame.height}px">
		<span><img src={frame.icon} alt="" />{frame.label}</span>
	</div>
{/if}
{#if error}
	<div class="file-drop-error" role="status">{error}</div>
{/if}

<style>
	.file-drop-feedback {
		position: absolute;
		z-index: 9;
		box-sizing: border-box;
		display: grid;
		place-items: center;
		pointer-events: none;
		opacity: 0;
		border: 0.12rem solid var(--gc-focus-color, #a3bfff);
		border-radius: 0.45rem;
		background: color-mix(in srgb, var(--gc-focus-color, #a3bfff) 17%, transparent);
		transition:
			opacity 100ms ease-out,
			left 100ms ease-out,
			top 100ms ease-out,
			width 100ms ease-out,
			height 100ms ease-out;
	}
	.file-drop-feedback.active {
		opacity: 1;
	}
	.file-drop-feedback span,
	.file-drop-error {
		border-radius: 0.35rem;
		padding: 0.25rem 0.45rem;
		background: var(--gc-color-bg-panel, #24262c);
		color: var(--gc-color-text);
		font-size: 0.75rem;
	}
	.file-drop-feedback span {
		display: flex;
		align-items: center;
		gap: 0.35rem;
		max-inline-size: 90%;
	}
	.file-drop-feedback img {
		inline-size: 0.9rem;
		block-size: 0.9rem;
	}
	.file-drop-error {
		position: absolute;
		z-index: 10;
		inset: auto 0.5rem 0.5rem;
		border: 0.075rem solid var(--gc-color-error, #eb7d76);
	}
	@media (prefers-reduced-motion: reduce) {
		.file-drop-feedback {
			transition: none;
		}
	}
</style>
