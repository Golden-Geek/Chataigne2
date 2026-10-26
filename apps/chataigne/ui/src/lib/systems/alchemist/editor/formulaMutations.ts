import type { UiNodeDto, WorkbenchSession } from 'golden_ui';

export type FormulaEditorSession = Pick<
	WorkbenchSession,
	| 'graph'
	| 'snapshotGeneration'
	| 'editOperations'
	| 'invokeAppOperation'
	| 'sendIntents'
	| 'selectedNodesIds'
	| 'selectionRevision'
	| 'graphTransitionRevision'
	| 'selectNodes'
	| 'selectNode'
	| 'requestNodeSelection'
>;
export interface FormulaEditorSource {
	session: FormulaEditorSession | null;
	formula: UiNodeDto | null;
	readOnly: boolean;
}
export type FormulaSaveStatus = 'saving' | 'saved' | 'error';

/** One captured Formula owner per confirmed session operation; no panel-owned queue or polling. */
export function createFormulaMutations(
	source: () => FormulaEditorSource,
	status: (status: FormulaSaveStatus) => void = () => {}
) {
	let disposed = false,
		sequence = 0;
	const pending = new Set<() => void>();
	const capture = () => {
		const value = source();
		if (disposed || !value.session) throw new Error('Formula editing requires a current session.');
		const session = value.session,
			formula = value.formula;
		const generation = session.snapshotGeneration;
		return {
			session,
			formula,
			graph: session.graph.state,
			generation,
			current: () => {
				const next = source();
				return (
					!disposed &&
					next.session === session &&
					session.snapshotGeneration === generation &&
					next.formula?.node_id === formula?.node_id &&
					next.formula?.uuid === formula?.uuid
				);
			}
		};
	};
	type Owner = ReturnType<typeof capture>;
	return {
		capture,
		async run<T>(operation: (owner: Owner) => Promise<T>, writing = true): Promise<T> {
			const owner = capture();
			if (writing && source().readOnly) throw new Error('Formula is read-only.');
			const work = owner.session.editOperations.open({
				current: () => owner.current() && (!writing || !source().readOnly),
				build: (scope) => ({
					accepted: true,
					async finish() {
						await scope.ready;
						if (!scope.current) throw new Error('Formula owner changed before dispatch.');
						result = await operation(owner);
					},
					async cancel() {}
				})
			});
			if (!work) throw new Error('Formula editing is busy or crossing a project barrier.');
			const ticket = ++sequence;
			let result: T;
			const cancel = () => {
				void work.cancel().catch(() => undefined);
			};
			pending.add(cancel);
			status('saving');
			try {
				await work.confirm();
				if (ticket === sequence && owner.current()) status('saved');
				return result!;
			} catch (error) {
				if (ticket === sequence && owner.current()) status('error');
				throw error;
			} finally {
				pending.delete(cancel);
			}
		},
		dispose() {
			disposed = true;
			for (const cancel of pending) cancel();
		}
	};
}
export type FormulaMutations = ReturnType<typeof createFormulaMutations>;
