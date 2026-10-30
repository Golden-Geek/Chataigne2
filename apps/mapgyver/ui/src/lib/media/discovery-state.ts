import type { SenderDiscoverySnapshot } from './generated/SenderDiscoverySnapshot';

// A completion event can reach the webview before the earlier refresh IPC reply.
export function newerDiscovery(
	current: SenderDiscoverySnapshot | null,
	next: SenderDiscoverySnapshot
): SenderDiscoverySnapshot {
	return current && current.revision > next.revision ? current : next;
}
