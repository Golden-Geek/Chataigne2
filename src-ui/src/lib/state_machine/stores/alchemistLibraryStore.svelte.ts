export interface LibraryEntry {
	id: string;
	label: string;
	category: string;
}

export class AlchemistLibraryStore {
	nodeCatalog = $state<LibraryEntry[]>([]);
	processorModelCatalog = $state<LibraryEntry[]>([]);
	query = $state('');

	get filteredNodes(): LibraryEntry[] {
		const query = this.query.trim().toLowerCase();
		if (!query) return this.nodeCatalog;
		return this.nodeCatalog.filter((entry) =>
			`${entry.label} ${entry.category}`.toLowerCase().includes(query)
		);
	}
}
