<script lang="ts">
	import { onMount, type Snippet } from 'svelte';
	import type { UiApplicationInfoDto } from '../../generated/rust_protocol/UiApplicationInfoDto';
	import { provideApplicationHost } from '../../host/application-context';
	import {
		discoverUiApplication,
		resolveRuntimeEndpoints,
		type RuntimeEndpoints
	} from '../../host/runtime-endpoints';
	import { configureApplication } from '../../store/application.svelte';

	let {
		expectedApplicationId,
		development = false,
		developmentBackendPort,
		children
	} = $props<{
		expectedApplicationId: string;
		development?: boolean;
		developmentBackendPort?: number;
		children?: Snippet;
	}>();
	let application = $state<UiApplicationInfoDto | null>(null);
	let endpoints = $state<RuntimeEndpoints | null>(null);
	let failure = $state<string | null>(null);
	provideApplicationHost({
		get application() {
			if (!application) throw new Error('Runtime application identity is not ready');
			return application;
		},
		get endpoints() {
			if (!endpoints) throw new Error('Runtime endpoints are not ready');
			return endpoints;
		}
	});

	onMount(() => {
		let stopped = false;
		let retry: ReturnType<typeof setTimeout> | undefined;
		const controller = new AbortController();
		const connect = async (): Promise<void> => {
			try {
				const resolved = resolveRuntimeEndpoints(window.location, {
					development,
					developmentBackendPort
				});
				const discovered = await discoverUiApplication(resolved.httpBaseUrl, {
					expectedApplicationId,
					signal: controller.signal
				});
				if (stopped) return;
				configureApplication(discovered);
				endpoints = resolved;
				application = discovered;
				failure = null;
			} catch (error) {
				if (stopped) return;
				failure = error instanceof Error ? error.message : String(error);
				retry = setTimeout(() => void connect(), 1000);
			}
		};
		void connect();
		return () => {
			stopped = true;
			controller.abort();
			clearTimeout(retry);
		};
	});
</script>

{#if application && endpoints}
	{#if children}{@render children()}{/if}
{:else}
	<main class="runtime-connection" role="status" aria-live="polite">
		<h1>{failure ? 'Application unavailable' : 'Opening workspace'}</h1>
		<p>{failure ?? 'Connecting to the application.'}</p>
		{#if failure}<p>The connection will retry automatically.</p>{/if}
	</main>
{/if}

<style>
	.runtime-connection {
		margin: 4rem auto;
		padding: 2rem;
		max-width: 40rem;
		font-family: system-ui, sans-serif;
	}
	h1 {
		font-size: 1.4rem;
	}
</style>
