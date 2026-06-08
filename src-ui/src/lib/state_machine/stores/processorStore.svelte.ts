import type { DiagnosticDto, ProcessorUiDto } from '../generated';

export class ProcessorStore {
	processorsById = $state(new Map<string, ProcessorUiDto>());
	diagnosticsById = $state(new Map<string, DiagnosticDto>());
	selectedProcessorId = $state<string | null>(null);

	replace(processors: ProcessorUiDto[], diagnostics: DiagnosticDto[]): void {
		this.processorsById = new Map(processors.map((processor) => [processor.id, processor]));
		this.diagnosticsById = new Map(diagnostics.map((diagnostic) => [diagnostic.id, diagnostic]));
	}

	select(processorId: string | null): void {
		this.selectedProcessorId = processorId;
	}

	get selected(): ProcessorUiDto | null {
		return this.selectedProcessorId
			? (this.processorsById.get(this.selectedProcessorId) ?? null)
			: null;
	}
}
