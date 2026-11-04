export type CanvasNavigationCommand = 'frame' | 'home' | 'zoom-in' | 'zoom-out';

/** Navigation only; domain editing, focus and active-pointer lifetime stay with the editor. */
export function canvasNavigationCommand(
	event: Pick<KeyboardEvent, 'key' | 'defaultPrevented' | 'ctrlKey' | 'metaKey' | 'altKey'>
): CanvasNavigationCommand | null {
	if (event.defaultPrevented || event.ctrlKey || event.metaKey || event.altKey) return null;
	const key = event.key.toLowerCase();
	if (key === 'f') return 'frame';
	if (key === 'h' || key === 'home') return 'home';
	if (key === '+' || key === '=') return 'zoom-in';
	if (key === '-') return 'zoom-out';
	return null;
}
