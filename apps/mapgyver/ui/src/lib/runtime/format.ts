/** Decimal allocation units match the MB/GB labels; the backend reports bytes. */
export function formatVideoMemory(bytes: number): string {
	return bytes >= 1_000_000_000
		? `${(bytes / 1_000_000_000).toFixed(2)} GB`
		: `${(bytes / 1_000_000).toFixed(1)} MB`;
}
