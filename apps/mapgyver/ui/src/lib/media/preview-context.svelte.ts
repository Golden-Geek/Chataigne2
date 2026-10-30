import { getContext, setContext } from 'svelte';
import { SvelteMap } from 'svelte/reactivity';
import { listen } from '@tauri-apps/api/event';
import { hasDesktopHost } from 'golden_ui/host/desktop';
import { createPreviewSession, createDesktopPreviewTransport } from 'golden_video_ui';
import type { PreviewStatus } from './generated/PreviewStatus';
import { PREVIEW_STATUS_EVENT } from './generated/constants';

const key = Symbol('mapgyver-media-preview');

export function provideMediaPreview() {
	const session = createPreviewSession(createDesktopPreviewTransport());
	const statuses = new SvelteMap<string, PreviewStatus>();
	const state = $state({ transparent: false });
	const owner = {
		session,
		statuses,
		state,
		mount() {
			let stopped = false;
			let unlisten: (() => void) | undefined;
			const unsubscribe = session.subscribe((value) => {
				state.transparent = value.installed && value.visibleViews > 0;
			});
			if (hasDesktopHost()) {
				void listen<PreviewStatus[]>(PREVIEW_STATUS_EVENT, (event) => {
					statuses.clear();
					for (const status of event.payload)
						statuses.set(JSON.stringify(status.reference), status);
				})
					.then((dispose) => {
						if (stopped) dispose();
						else {
							unlisten = dispose;
							void session.connect();
						}
					})
					.catch(() => {
						state.transparent = false;
					});
			} else {
				void session.connect();
			}
			return () => {
				stopped = true;
				unsubscribe();
				unlisten?.();
				void session.dispose();
			};
		}
	};
	setContext(key, owner);
	return owner;
}

export const useMediaPreview = () => getContext<ReturnType<typeof provideMediaPreview>>(key);
