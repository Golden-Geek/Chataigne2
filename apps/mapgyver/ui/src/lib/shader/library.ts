import { invoke } from '@tauri-apps/api/core';
import { appState } from 'golden_ui';
import type { LibraryShader } from '../media/generated/LibraryShader';
import type { ShaderLibraryEntry } from '../media/generated/ShaderLibraryEntry';

const ISF_ENTRY = 'application/x-mapgyver-isf-library-entry';
let drag: { id: string; token: string; current: () => boolean } | null = null;

export const browseIsf = (refresh: boolean) =>
	invoke<ShaderLibraryEntry[]>('plugin:mapgyver-shader-library|browse_isf', { refresh });

export const loadIsf = (id: string) =>
	invoke<LibraryShader>('plugin:mapgyver-shader-library|load_isf', { id });

export function beginIsfDrag(event: DragEvent, id: string): void {
	const transfer = event.dataTransfer;
	const session = appState.session;
	if (!transfer || !session || session.status !== 'connected' || session.loading.visible) return;
	const generation = session.snapshotGeneration;
	const token = crypto.randomUUID();
	drag = {
		id,
		token,
		current: () => appState.session === session && session.snapshotGeneration === generation
	};
	transfer.effectAllowed = 'copy';
	transfer.setData(ISF_ENTRY, token);
}

export function endIsfDrag(): void {
	drag = null;
}

export function matchesIsfDrag(event: DragEvent): boolean {
	return Boolean(drag?.current() && event.dataTransfer?.types.includes(ISF_ENTRY));
}

export function readIsfDrop(event: DragEvent): string | null {
	return drag?.current() && event.dataTransfer?.getData(ISF_ENTRY) === drag.token ? drag.id : null;
}
