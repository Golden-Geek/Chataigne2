import type { UiProjectFileSpec } from '../types';
import { parseProjectFileSpec } from '../project-file-spec';

type ProjectFileFormat = Pick<UiProjectFileSpec, 'display_name' | 'extension'>;

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

export const projectFileFormatState = $state<{ spec: ProjectFileFormat | null }>({ spec: null });

export const setProjectFileFormat = (next: UiProjectFileSpec): void => {
	try {
		const spec = parseProjectFileSpec(next);
		projectFileFormatState.spec = { display_name: spec.display_name, extension: spec.extension };
	} catch (error) {
		projectFileFormatState.spec = null;
		throw error;
	}
};

export const resetProjectFileFormat = (): void => {
	projectFileFormatState.spec = null;
};

export const isProjectFileFormatReady = (): boolean => projectFileFormatState.spec !== null;

const requireProjectFileFormat = (): ProjectFileFormat => {
	const format = projectFileFormatState.spec;
	if (!format) throw new Error('Project file format is unavailable before the runtime snapshot');
	return format;
};

export const projectFileDialogExtensions = (): string[] => [requireProjectFileFormat().extension];

export const projectFileExtensionLabel = (): string =>
	projectFileFormatState.spec === null ? '' : `.${projectFileFormatState.spec.extension}`;

export const projectFileFilterLabel = (): string => requireProjectFileFormat().display_name;

export const projectFileUploadAccept = (): string => projectFileExtensionLabel();

export const projectFileDialogTitle = (action: 'open' | 'save'): string => {
	const format = requireProjectFileFormat();
	const article = startsWithVowelSound(format.display_name) ? 'an' : 'a';
	const verb = action === 'open' ? 'Open' : 'Save';
	return `${verb} ${article} ${format.display_name}`;
};

export const normalizeProjectFilePath = (value: string): string => {
	const format = requireProjectFileFormat();
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
	if (extension === format.extension.toLowerCase()) {
		return trimmed;
	}

	return `${directory}${baseName}.${format.extension}`;
};
