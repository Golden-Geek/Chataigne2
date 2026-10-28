import { afterEach, describe, expect, it, vi } from 'vitest';
import { applicationState, configureApplication } from 'golden_ui/store/application.svelte';
import {
	clearPersistedUiState,
	loadPersistedDockLayout,
	loadPersistedLastProjectPath,
	loadPersistedSelection,
	savePersistedDockLayout,
	savePersistedLastProjectPath,
	savePersistedSelection
} from 'golden_ui/store/ui-persistence';
import type { SerializedDockview } from 'dockview-core';
import {
	browserProjectDirectoryLabel,
	initializeProjectFileState,
	projectFileState
} from 'golden_ui/store/project-files.svelte';

const app = (id: string) => ({
	id,
	display_name: id,
	version: '1.0.0',
	data_directory_name: `Data ${id}`
});
const storage = () => {
	const entries = new Map<string, string>();
	return {
		entries,
		getItem: vi.fn((key: string) => entries.get(key) ?? null),
		setItem: vi.fn((key: string, value: string) => {
			entries.set(key, value);
		}),
		removeItem: vi.fn((key: string) => {
			entries.delete(key);
		})
	};
};

afterEach(() => {
	applicationState.info = null;
	projectFileState.currentPath = null;
	projectFileState.lastOpenedPath = null;
	vi.unstubAllGlobals();
});

describe('application workbench persistence', () => {
	it('does not read or write UI state before discovery provides an application identity', () => {
		const local = storage();
		vi.stubGlobal('window', { localStorage: local });
		expect(loadPersistedLastProjectPath()).toBeNull();
		expect(loadPersistedSelection()).toEqual([]);
		savePersistedLastProjectPath('/unidentified');
		savePersistedSelection([1]);
		clearPersistedUiState();
		expect(local.getItem).not.toHaveBeenCalled();
		expect(local.setItem).not.toHaveBeenCalled();
		expect(local.removeItem).not.toHaveBeenCalled();
	});

	it('restores app-specific recent paths only after identity configuration', () => {
		const local = storage();
		vi.stubGlobal('window', { localStorage: local });
		configureApplication(app('first/profile'));
		savePersistedLastProjectPath('/first.noisette');
		expect(projectFileState.lastOpenedPath).toBeNull();
		initializeProjectFileState();
		expect(projectFileState.lastOpenedPath).toBe('/first.noisette');
		configureApplication(app('second:profile'));
		savePersistedLastProjectPath('/second.mapgyver');
		initializeProjectFileState();
		expect(projectFileState.lastOpenedPath).toBe('/second.mapgyver');
		expect(browserProjectDirectoryLabel()).toBe('~/Documents/Data second:profile');
		configureApplication(app('first/profile'));
		initializeProjectFileState();
		expect(projectFileState.lastOpenedPath).toBe('/first.noisette');
	});

	it('keeps selection and recent paths independent and clears only the current app', () => {
		const local = storage();
		vi.stubGlobal('window', { localStorage: local });
		configureApplication(app('isolation/one'));
		const layout = { grid: {}, panels: {} } as unknown as SerializedDockview;
		savePersistedDockLayout(layout);
		savePersistedSelection([1, 2]);
		savePersistedLastProjectPath('/one');
		configureApplication(app('isolation:two'));
		expect(loadPersistedDockLayout()).toBeNull();
		expect(loadPersistedSelection()).toEqual([]);
		expect(loadPersistedLastProjectPath()).toBeNull();
		savePersistedSelection([3]);
		savePersistedLastProjectPath('/two');
		configureApplication(app('isolation/one'));
		expect(loadPersistedDockLayout()).toEqual(layout);
		expect(loadPersistedSelection()).toEqual([1, 2]);
		expect(loadPersistedLastProjectPath()).toBe('/one');
		clearPersistedUiState();
		expect(loadPersistedDockLayout()).toBeNull();
		expect(loadPersistedSelection()).toEqual([]);
		configureApplication(app('isolation:two'));
		expect(loadPersistedSelection()).toEqual([3]);
		expect(loadPersistedLastProjectPath()).toBe('/two');
	});
});
