import { invoke } from '@tauri-apps/api/core';
import { appState } from 'golden_ui';
import type { LibraryShader } from '../media/generated/LibraryShader';
import type { ShaderLibraryEntry } from '../media/generated/ShaderLibraryEntry';
import type { ShaderLibraryPage } from '../media/generated/ShaderLibraryPage';

export type OnlineShaderSource = 'isf' | 'fragcoord';
interface ShaderSelection {
	source: OnlineShaderSource;
	id: string;
}

const SHADER_ENTRY = 'application/x-mapgyver-shader-library-entry';
let drag: { selection: ShaderSelection; token: string; current: () => boolean } | null = null;

export const browseIsf = (refresh: boolean) =>
	invoke<ShaderLibraryEntry[]>('plugin:mapgyver-shader-library|browse_isf', { refresh });

export const browseFragCoord = (query: string, offset: number) =>
	invoke<ShaderLibraryPage>('plugin:mapgyver-shader-library|browse_fragcoord', { query, offset });

export const loadLibraryShader = ({ source, id }: ShaderSelection) =>
	invoke<LibraryShader>(`plugin:mapgyver-shader-library|load_${source}`, { id });

export function beginShaderDrag(event: DragEvent, selection: ShaderSelection): void {
	const transfer = event.dataTransfer;
	const session = appState.session;
	if (!transfer || !session || session.status !== 'connected' || session.loading.visible) return;
	const generation = session.snapshotGeneration;
	const token = crypto.randomUUID();
	drag = {
		selection,
		token,
		current: () => appState.session === session && session.snapshotGeneration === generation
	};
	transfer.effectAllowed = 'copy';
	transfer.setData(SHADER_ENTRY, token);
}

export function endShaderDrag(): void {
	drag = null;
}

export function matchesShaderDrag(event: DragEvent): boolean {
	return Boolean(drag?.current() && event.dataTransfer?.types.includes(SHADER_ENTRY));
}

export function readShaderDrop(event: DragEvent): ShaderSelection | null {
	return drag?.current() && event.dataTransfer?.getData(SHADER_ENTRY) === drag.token
		? drag.selection
		: null;
}
