import type { UiTransportConnectionState } from '../../transport';

export type WorkbenchConnectionStatus = 'disconnected' | 'connecting' | 'connected';

export const workbenchConnectionStatus = (
	transportState: UiTransportConnectionState,
	hasLoadedSnapshot: boolean
): WorkbenchConnectionStatus => {
	if (transportState === 'connected') {
		return hasLoadedSnapshot ? 'connected' : 'connecting';
	}
	if (transportState === 'connecting' || transportState === 'reconnecting') {
		return 'connecting';
	}
	return 'disconnected';
};

export const formatReconnectDelay = (delayMs: number): string => {
	const seconds = Math.max(1, Math.ceil(delayMs / 1000));
	return `${seconds}s`;
};
