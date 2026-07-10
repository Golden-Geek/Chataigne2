import type {
  FormulaCatalogItem,
  FormulaDelta,
  FormulaId,
  FormulaSnapshot,
  FormulaSurfaceInput,
  FormulaSurfaceOutput,
  SurfaceItemId,
} from "./types";

export class FormulaRevisionConflict extends Error {}

export class FormulaStore {
  readonly catalog = new Map<FormulaId, FormulaCatalogItem>();
  readonly inputs = new Map<SurfaceItemId, FormulaSurfaceInput>();
  readonly outputs = new Map<SurfaceItemId, FormulaSurfaceOutput>();
  readonly runtimeOutputs = new Map<SurfaceItemId, unknown>();

  readonly formulaId: FormulaId;
  revision: number;
  catalogRevision = 0;
  surfaceRevision = 0;
  outputRevision = 0;

  constructor(snapshot: FormulaSnapshot) {
    this.formulaId = snapshot.formulaId;
    this.revision = snapshot.revision;
    for (const item of snapshot.catalog) this.catalog.set(item.id, item);
    for (const input of snapshot.inputs) this.inputs.set(input.id, input);
    for (const output of snapshot.outputs) this.outputs.set(output.id, output);
    for (const [id, value] of Object.entries(snapshot.runtimeOutputs ?? {})) {
      this.runtimeOutputs.set(id as SurfaceItemId, value);
    }
  }

  apply(delta: FormulaDelta): void {
    if (delta.before !== this.revision || delta.after <= delta.before) {
      throw new FormulaRevisionConflict(
        `expected formula revision ${this.revision}, received ${delta.before} -> ${delta.after}`,
      );
    }
    let catalogChanged = false;
    let surfaceChanged = false;
    let outputChanged = false;
    for (const change of delta.changes) {
      switch (change.kind) {
        case "catalog-upserted":
          this.catalog.set(change.item.id, change.item);
          catalogChanged = true;
          break;
        case "catalog-removed":
          catalogChanged = this.catalog.delete(change.formulaId) || catalogChanged;
          break;
        case "surface-input-upserted":
          this.inputs.set(change.input.id, change.input);
          surfaceChanged = true;
          break;
        case "surface-input-removed":
          surfaceChanged = this.inputs.delete(change.inputId) || surfaceChanged;
          break;
        case "surface-output-upserted":
          this.outputs.set(change.output.id, change.output);
          surfaceChanged = true;
          break;
        case "surface-output-removed":
          this.runtimeOutputs.delete(change.outputId);
          surfaceChanged = this.outputs.delete(change.outputId) || surfaceChanged;
          break;
        case "runtime-output":
          if (!this.outputs.has(change.outputId)) {
            throw new Error(`runtime output references missing surface item ${change.outputId}`);
          }
          if (!Object.is(this.runtimeOutputs.get(change.outputId), change.value)) {
            this.runtimeOutputs.set(change.outputId, change.value);
            outputChanged = true;
          }
          break;
      }
    }
    this.revision = delta.after;
    if (catalogChanged) this.catalogRevision += 1;
    if (surfaceChanged) this.surfaceRevision += 1;
    if (outputChanged) this.outputRevision += 1;
  }
}
