<script lang="ts">
	import type { FileExplorerEntry } from '../../../generated/file-explorer/FileExplorerEntry';
	import { readProjectThumbnail } from '../../../host/file-explorer';
	import fileIcon from '../../../style/icons/parameter/file.svg';
	import folderIcon from '../../../style/icons/node/folder.svg';

	const { entry, root }: { entry: FileExplorerEntry; root: string } = $props();
	let target = $state<HTMLSpanElement>();
	let thumbnail = $state<string>();

	$effect(() => {
		const element = target;
		const path = entry.path;
		const directory = root;
		if (!element || entry.directory || !/\.(png|jpe?g|webp)$/i.test(entry.name)) return;
		let active = true;
		let objectUrl: string | undefined;
		const observer = new IntersectionObserver((changes) => {
			if (!changes.some((change) => change.isIntersecting)) return;
			observer.disconnect();
			void readProjectThumbnail(directory, path)
				.then((bytes) => {
					if (!active) return;
					objectUrl = URL.createObjectURL(new Blob([bytes], { type: 'image/png' }));
					thumbnail = objectUrl;
				})
				.catch(() => {});
		});
		observer.observe(element);
		return () => {
			active = false;
			observer.disconnect();
			if (objectUrl) URL.revokeObjectURL(objectUrl);
			thumbnail = undefined;
		};
	});
</script>

<span class="thumbnail" bind:this={target} data-image-thumbnail={thumbnail ? '' : undefined}>
	<img
		class:preview={Boolean(thumbnail)}
		src={thumbnail ?? (entry.directory ? folderIcon : fileIcon)}
		alt=""
		draggable="false" />
</span>

<style>
	.thumbnail {
		display: grid;
		place-items: center;
		aspect-ratio: 4 / 3;
		inline-size: 100%;
		overflow: hidden;
	}
	img {
		inline-size: 40%;
		block-size: 60%;
		object-fit: contain;
		pointer-events: none;
	}
	img.preview {
		inline-size: 100%;
		block-size: 100%;
	}
</style>
