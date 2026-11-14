/** Select by the current intent/authority; cached off-page content cannot invent a selection.
 * Callers retain their own session/project fences and bounded visible definitions.
 */
export function selectedEditorItem<T, Id extends string | number>(
	selected: Id | null | undefined,
	visible: readonly T[],
	independent: T | null | undefined,
	identity: (item: T) => Id
): T | null {
	if (selected == null) return null;
	return (
		visible.find((item) => identity(item) === selected) ??
		(independent && identity(independent) === selected ? independent : null)
	);
}
