import { getContext, onMount, setContext } from 'svelte';
import { createVideoStatusState, VideoStatusController } from './status';
import { createDesktopVideoStatusTransport } from './transport';
const key = Symbol('mapgyver-video-status');

/** One transport subscription for every footer and mounted outliner row in this window. */
export function provideVideoStatus() {
	const state = $state(createVideoStatusState());
	const controller = new VideoStatusController(createDesktopVideoStatusTransport(), state);
	onMount(() => controller.connect());
	setContext(key, state);
	return state;
}
export const useVideoStatus = () => getContext<ReturnType<typeof provideVideoStatus>>(key);
