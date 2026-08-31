/** Framework-neutral browser adapter for generated acorde WASM bindings. */

export type NoteAddress = string;

export interface WasmBindings {
  parse_musicxml(xml: string): string;
  compute_layout_ex(scoreJson: string, configJson: string): string;
  render_score_svg_with_layout(scoreJson: string, layoutJson: string, optionsJson: string): string;
  render_score_metadata(scoreJson: string, layoutJson: string, optionsJson: string): string;
  analyze_score(scoreJson: string): string;
}

export interface ScoreWorkspaceOptions {
  layout?: Record<string, unknown>;
  render?: Record<string, unknown>;
}

export interface SelectionStore {
  get(): NoteAddress | null;
  set(address: NoteAddress | null): void;
  subscribe(listener: (address: NoteAddress | null) => void): () => void;
}

export interface WorkspaceSnapshot {
  scoreJson: string;
  layoutJson: string;
  metadata: Record<string, unknown>;
  analysis: Record<string, unknown>;
}

export function createSelectionStore(initial: NoteAddress | null = null): SelectionStore {
  let current = initial;
  const listeners = new Set<(address: NoteAddress | null) => void>();
  return {
    get: () => current,
    set: (address) => {
      if (address === current) return;
      current = address;
      listeners.forEach((listener) => listener(current));
    },
    subscribe: (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
}

export class AcordeWorkspace {
  readonly selection: SelectionStore;
  private scoreJson = "";
  private layoutJson = "";

  constructor(
    private readonly wasm: WasmBindings,
    private readonly options: ScoreWorkspaceOptions = {},
    selection: SelectionStore = createSelectionStore(),
  ) {
    this.selection = selection;
  }

  loadMusicXml(xml: string): void {
    this.scoreJson = this.wasm.parse_musicxml(xml);
    this.layoutJson = this.wasm.compute_layout_ex(
      this.scoreJson,
      JSON.stringify(this.options.layout ?? {}),
    );
    this.selection.set(null);
  }

  renderSvg(): string {
    this.assertLoaded();
    return this.wasm.render_score_svg_with_layout(
      this.scoreJson,
      this.layoutJson,
      JSON.stringify(this.options.render ?? {}),
    );
  }

  metadata(): Record<string, unknown> {
    this.assertLoaded();
    return JSON.parse(this.wasm.render_score_metadata(
      this.scoreJson,
      this.layoutJson,
      JSON.stringify(this.options.render ?? {}),
    )) as Record<string, unknown>;
  }

  analyze(): Record<string, unknown> {
    this.assertLoaded();
    return JSON.parse(this.wasm.analyze_score(this.scoreJson)) as Record<string, unknown>;
  }

  snapshot(): WorkspaceSnapshot {
    return {
      scoreJson: this.scoreJson,
      layoutJson: this.layoutJson,
      metadata: this.metadata(),
      analysis: this.analyze(),
    };
  }

  private assertLoaded(): void {
    if (!this.scoreJson || !this.layoutJson) {
      throw new Error("A score must be loaded before rendering or analysis");
    }
  }
}
