import type { FileExplorerCursor } from '../generated/file-explorer/FileExplorerCursor';
import type { FileExplorerPage } from '../generated/file-explorer/FileExplorerPage';
import { invoke } from '@tauri-apps/api/core';

export const browseProjectFiles = (
	project: string | null,
	folder: string | null,
	directory?: string,
	after?: FileExplorerCursor,
	suffixes?: readonly string[]
): Promise<FileExplorerPage | null> =>
	invoke('file_explorer_browse', { project, folder, directory, after, suffixes });

export const readProjectThumbnail = (root: string, path: string): Promise<ArrayBuffer> =>
	invoke('file_explorer_thumbnail', { root, path });
