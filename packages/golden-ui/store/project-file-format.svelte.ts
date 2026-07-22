import type { UiProjectFileSpec } from '../types';

const DEFAULT_PROJECT_FILE_SPEC: UiProjectFileSpec = {
	display_name: 'Project',
	extension: 'json',
	current_path: null
};

const normalizeExtension = (value: string | null | undefined): string => {
	const normalized = value?.trim().replace(/^\.+/, '').toLowerCase() ?? '';
	return normalized.length > 0 ? normalized : DEFAULT_PROJECT_FILE_SPEC.extension;
};

const normalizeDisplayName = (value: string | null | undefined): string => {
	const normalized = value?.trim() ?? '';
	if (normalized.length === 0) {
		return DEFAULT_PROJECT_FILE_SPEC.display_name;
	}

	return normalized.replace(/\s+files?$/i, '').trim() || DEFAULT_PROJECT_FILE_SPEC.display_name;
};

const startsWithVowelSound = (value: string): boolean => /^[aeiou]/i.test(value.trim());

const splitProjectPath = (
	value: string
): {
	directory: string;
	fileName: string;
} => {
	const separatorIndex = Math.max(value.lastIndexOf('/'), value.lastIndexOf('\\'));
	if (separatorIndex < 0) {
		return {
			directory: '',
			fileName: value
		};
	}

	return {
		directory: value.slice(0, separatorIndex + 1),
		fileName: value.slice(separatorIndex + 1)
	};
};

export const projectFileFormatState = $state({
	display_name: DEFAULT_PROJECT_FILE_SPEC.display_name,
	extension: DEFAULT_PROJECT_FILE_SPEC.extension
});

export const setProjectFileFormat = (next: UiProjectFileSpec | null | undefined): void => {
	projectFileFormatState.display_name = normalizeDisplayName(next?.display_name);
	projectFileFormatState.extension = normalizeExtension(next?.extension);
};

export const resetProjectFileFormat = (): void => {
	setProjectFileFormat(DEFAULT_PROJECT_FILE_SPEC);
};

export const projectFileDialogExtensions = (): string[] => [projectFileFormatState.extension];

export const projectFileExtensionLabel = (): string => `.${projectFileFormatState.extension}`;

export const projectFileFilterLabel = (): string => projectFileFormatState.display_name;

export const projectFileUploadAccept = (): string => projectFileExtensionLabel();

export const projectFileDialogTitle = (action: 'open' | 'save'): string => {
	const article = startsWithVowelSound(projectFileFormatState.display_name) ? 'an' : 'a';
	const verb = action === 'open' ? 'Open' : 'Save';
	return `${verb} ${article} ${projectFileFormatState.display_name}`;
};

export const normalizeProjectFilePath = (value: string): string => {
	const trimmed = value.trim();
	if (trimmed.length === 0) {
		return trimmed;
	}

	const { directory, fileName } = splitProjectPath(trimmed);
	if (fileName.length === 0) {
		return trimmed;
	}

	const lastDotIndex = fileName.lastIndexOf('.');
	const baseName = lastDotIndex > 0 ? fileName.slice(0, lastDotIndex) : fileName;
	const extension = lastDotIndex > 0 ? fileName.slice(lastDotIndex + 1).toLowerCase() : '';
	if (extension === projectFileFormatState.extension) {
		return trimmed;
	}

	return `${directory}${baseName}.${projectFileFormatState.extension}`;
};
