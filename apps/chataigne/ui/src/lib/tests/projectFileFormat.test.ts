import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { appState, createWorkbenchSession } from 'golden_ui/store/workbench.svelte';
import {
	isProjectFileFormatReady,
	normalizeProjectFilePath,
	projectFileDialogExtensions,
	projectFileFilterLabel,
	projectFileUploadAccept,
	resetProjectFileFormat,
	setProjectFileFormat
} from 'golden_ui/store/project-file-format.svelte';
import {
	attemptSaveProjectFile,
	saveProjectFileAs,
	openProjectFile,
	projectFileState,
	uploadProjectFileAndLoad
} from 'golden_ui/store/project-files.svelte';
import { handleCommandShortcut } from 'golden_ui/store/commands.svelte';
import { configureApplication } from 'golden_ui/store/application.svelte';
import { openDesktopFileDialog, saveDesktopFileDialog } from 'golden_ui/host/desktop';
import type { UiClient, UiProjectFileSpec, UiSnapshot } from 'golden_ui/types';
import { snapshot } from './fixtures/workbench-graph';

vi.mock('golden_ui/host/desktop', async (original) => ({
	...(await original<typeof import('golden_ui/host/desktop')>()),
	hasDesktopHost: vi.fn(() => true),
	openDesktopFileDialog: vi.fn(async () => null),
	saveDesktopFileDialog: vi.fn(async () => null)
}));

const spec: UiProjectFileSpec = {
	display_name: 'MapGyver',
	extension: 'mapgyver',
	current_path: null
};
const authoredSnapshot = (): UiSnapshot => ({ ...snapshot(1, 0), project_file: spec });

beforeEach(() => {
	vi.clearAllMocks();
	resetProjectFileFormat();
	projectFileState.currentPath = null;
	projectFileState.busy = false;
});
afterEach(() => {
	appState.session = null;
	resetProjectFileFormat();
	vi.restoreAllMocks();
});

describe('project file format readiness', () => {
	it('has no upload filter or operational path normalization before a snapshot', () => {
		expect(isProjectFileFormatReady()).toBe(false);
		expect(projectFileUploadAccept()).toBe('');
		expect(() => projectFileDialogExtensions()).toThrow('before the runtime snapshot');
		expect(() => normalizeProjectFilePath('show')).toThrow('before the runtime snapshot');
	});

	it('blocks pre-snapshot Ctrl+S and API file actions before dialogs or file reads', async () => {
		let finishSnapshot!: (value: UiSnapshot) => void;
		const pending = new Promise<UiSnapshot>((resolve) => {
			finishSnapshot = resolve;
		});
		const session = createWorkbenchSession({
			enableGlobalShortcuts: false,
			transportFactory: () => ({ snapshot: () => pending }) as unknown as UiClient
		});
		appState.session = session;
		const cleanup = session.mount();
		const event = {
			key: 's',
			code: 'KeyS',
			ctrlKey: true,
			metaKey: false,
			shiftKey: false,
			altKey: false,
			preventDefault: vi.fn()
		} as unknown as KeyboardEvent;
		const file = { name: 'show.mapgyver', size: 1, text: vi.fn() } as unknown as File;
		vi.spyOn(console, 'warn').mockImplementation(() => {});
		try {
			await expect(handleCommandShortcut(event)).resolves.toBe(false);
			expect(event.preventDefault).toHaveBeenCalledOnce();
			const result = await attemptSaveProjectFile();
			expect(result.ok).toBe(false);
			expect(result.message).toContain('until the runtime project file format is ready');
			await expect(openProjectFile()).resolves.toBe(false);
			await expect(uploadProjectFileAndLoad(file)).resolves.toBe(false);
			expect(file.text).not.toHaveBeenCalled();
			expect(saveDesktopFileDialog).not.toHaveBeenCalled();
			expect(openDesktopFileDialog).not.toHaveBeenCalled();
			expect(projectFileState.currentPath).toBeNull();
			expect(projectFileState.busy).toBe(false);
		} finally {
			cleanup();
			finishSnapshot(authoredSnapshot());
			await Promise.resolve();
		}
	});

	it('uses only the explicit format supplied by an applied MapGyver snapshot', async () => {
		const session = createWorkbenchSession({
			enableGlobalShortcuts: false,
			transportFactory: () => ({ snapshot: async () => authoredSnapshot() }) as unknown as UiClient
		});
		appState.session = session;
		expect(await session.refreshSnapshot()).toBe(true);
		expect(isProjectFileFormatReady()).toBe(true);
		expect(projectFileDialogExtensions()).toEqual(['mapgyver']);
		expect(projectFileFilterLabel()).toBe('MapGyver');
		expect(projectFileUploadAccept()).toBe('.mapgyver');
		expect(normalizeProjectFilePath('/work/show.json')).toBe('/work/show.mapgyver');
		await saveProjectFileAs();
		expect(saveDesktopFileDialog).toHaveBeenCalledWith({
			suggestedPath: 'Untitled.mapgyver',
			allowedExtensions: ['mapgyver'],
			filterLabel: 'MapGyver',
			title: 'Save a MapGyver',
			logTag: 'project-files'
		});
	});

	it('clears readiness for malformed metadata, a new session or a different application', () => {
		setProjectFileFormat(spec);
		expect(() => setProjectFileFormat({ ...spec, extension: '' })).toThrow('metadata is invalid');
		expect(isProjectFileFormatReady()).toBe(false);
		setProjectFileFormat(spec);
		createWorkbenchSession({ transportFactory: () => ({}) as UiClient });
		expect(isProjectFileFormatReady()).toBe(false);
		setProjectFileFormat(spec);
		configureApplication({
			id: 'format-switch',
			display_name: 'Other',
			version: '1.0',
			data_directory_name: 'Other'
		});
		expect(isProjectFileFormatReady()).toBe(false);
	});

	it('clears the authoritative format when the owning session is unmounted', async () => {
		let finishSnapshot!: (value: UiSnapshot) => void;
		const pending = new Promise<UiSnapshot>((resolve) => {
			finishSnapshot = resolve;
		});
		const session = createWorkbenchSession({
			enableGlobalShortcuts: false,
			transportFactory: () => ({ snapshot: () => pending }) as unknown as UiClient
		});
		const cleanup = session.mount();
		setProjectFileFormat(spec);
		cleanup();
		expect(isProjectFileFormatReady()).toBe(false);
		finishSnapshot(authoredSnapshot());
		await Promise.resolve();
	});
});
