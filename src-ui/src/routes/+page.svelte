<script lang="ts">
	import { onMount } from "svelte";
	import MainComponent from "$lib/MainComponent.svelte";

	type AppWindow = {
		close: () => Promise<void>;
		minimize: () => Promise<void>;
		toggleMaximize: () => Promise<void>;
		isMaximized: () => Promise<boolean>;
		onResized: (handler: () => void) => Promise<() => void>;
	};

	let isWindowMaximized = $state(false);
	let hasTauriWindowApi = $state(false);

	const getAppWindow = (): AppWindow | undefined =>
		window.__TAURI__?.window.getCurrentWindow();

	const refreshMaximizeState = async (): Promise<void> => {
		const appWindow = getAppWindow();
		if (!appWindow) {
			isWindowMaximized = false;
			return;
		}

		isWindowMaximized = await appWindow.isMaximized();
	};

	const minimizeWindow = async (): Promise<void> => {
		await getAppWindow()?.minimize();
	};

	const toggleWindowMaximize = async (): Promise<void> => {
		await getAppWindow()?.toggleMaximize();
		await refreshMaximizeState();
	};

	const closeWindow = async (): Promise<void> => {
		await getAppWindow()?.close();
	};

	onMount(() => {
		const appWindow = getAppWindow();
		if (!appWindow) {
			hasTauriWindowApi = false;
			return;
		}

		hasTauriWindowApi = true;
		void refreshMaximizeState();

		let unlistenResize: (() => void) | undefined;
		void appWindow
			.onResized(() => {
				void refreshMaximizeState();
			})
			.then((unlisten) => {
				unlistenResize = unlisten;
			});

		return () => {
			unlistenResize?.();
		};
	});
</script>

<div class="gc-main">
	<div class="gc-header" data-tauri-drag-region>
		<div class="app-title">Chataigne 2.0.0</div>
		<div class="spacer"></div>
		<div class="app-buttons" data-no-drag>
			<button
				type="button"
				class="minimize-app"
				aria-label="Minimize app"
				disabled={!hasTauriWindowApi}
				onclick={minimizeWindow}>_</button
			>
			<button
				type="button"
				class="maximize-app"
				aria-label={isWindowMaximized ? "Restore app" : "Maximize app"}
				disabled={!hasTauriWindowApi}
				onclick={toggleWindowMaximize}>{isWindowMaximized ? "[]" : "[ ]"}</button
			>
			<button
				type="button"
				class="close-app"
				aria-label="Close app"
				disabled={!hasTauriWindowApi}
				onclick={closeWindow}>x</button
			>
		</div>
	</div>
	<div class="gc-content">
		<MainComponent />
	</div>
	<div class="gc-footer"></div>
</div>
