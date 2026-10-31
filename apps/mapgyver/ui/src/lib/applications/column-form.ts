import type { ColumnSnapshot } from '../media/generated/ColumnSnapshot';
import type { ColumnApplication } from '../media/generated/ColumnApplication';
import type { MatrixEdit } from '../media/generated/MatrixEdit';

function number(data: FormData, key: string) {
	const raw = data.get(key);
	if (typeof raw !== 'string' || !raw.trim() || !Number.isFinite(Number(raw)))
		throw new Error(`Enter ${key}`);
	return Number(raw);
}
function configure(column: ColumnSnapshot, applications: ColumnApplication[]): MatrixEdit {
	return {
		kind: 'ConfigureColumn',
		column: column.id,
		definition: { ...column.definition, applications }
	};
}
export function columnDistances(column: ColumnSnapshot, data: FormData) {
	return configure(
		column,
		column.definition.applications.map((entry) => ({
			...entry,
			pre: number(data, `pre:${entry.application}`),
			post: number(data, `post:${entry.application}`)
		}))
	);
}
export function associateApplication(column: ColumnSnapshot, application: string, data: FormData) {
	return configure(column, [
		...column.definition.applications,
		{ application, pre: number(data, 'pre'), post: number(data, 'post') }
	]);
}
export function removeAssociation(column: ColumnSnapshot, application: string) {
	return configure(
		column,
		column.definition.applications.filter((entry) => entry.application !== application)
	);
}
