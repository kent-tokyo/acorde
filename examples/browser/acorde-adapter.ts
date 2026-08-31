/** Framework-neutral browser adapter for generated acorde WASM bindings. */

export type NoteAddress = string;

export type WorkspaceOperation = "parse" | "layout" | "render" | "metadata" | "analysis";

/** Structured, host-facing error for showing a repair hint without parsing strings. */
export class AcordeWorkspaceError extends Error {
  constructor(
    readonly operation: WorkspaceOperation,
    message: string,
    readonly cause?: unknown,
  ) {
    super(`${operation} failed: ${message}`);
    this.name = "AcordeWorkspaceError";
  }
}

export interface WasmBindings {
  parse_musicxml(xml: string): string;
  compute_layout_ex(scoreJson: string, configJson: string): string;
  render_score_svg_with_layout(scoreJson: string, layoutJson: string, optionsJson: string): string;
  render_score_svg_row(
    scoreJson: string,
    layoutJson: string,
    rowIndex: number,
    optionsJson: string,
  ): string;
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
  revision: number;
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
  private revisionNumber = 0;
  private readonly layoutCache = new Map<string, string>();
  private readonly renderCache = new Map<string, string>();
  private readonly metadataCache = new Map<string, Record<string, unknown>>();
  private readonly analysisCache = new Map<string, Record<string, unknown>>();

  constructor(
    private readonly wasm: WasmBindings,
    private readonly options: ScoreWorkspaceOptions = {},
    selection: SelectionStore = createSelectionStore(),
  ) {
    this.selection = selection;
  }

  loadMusicXml(xml: string): void {
    let nextScore: string;
    let nextLayout: string;
    try {
      nextScore = this.wasm.parse_musicxml(xml);
    } catch (cause) {
      throw this.toWorkspaceError("parse", cause);
    }
    const layoutOptionsJson = JSON.stringify(this.options.layout ?? {});
    try {
      nextLayout = this.wasm.compute_layout_ex(nextScore, layoutOptionsJson);
    } catch (cause) {
      throw this.toWorkspaceError("layout", cause);
    }
    this.layoutCache.clear();
    this.renderCache.clear();
    this.metadataCache.clear();
    this.analysisCache.clear();
    this.revisionNumber += 1;
    const layoutKey = `${this.revisionNumber}:${layoutOptionsJson}`;
    this.scoreJson = nextScore;
    this.layoutJson = nextLayout;
    this.layoutCache.set(layoutKey, this.layoutJson);
    this.selection.set(null);
  }

  get revision(): number {
    return this.revisionNumber;
  }

  renderSvg(): string {
    this.assertLoaded();
    const optionsJson = JSON.stringify(this.options.render ?? {});
    const key = `${this.revisionNumber}:${this.layoutJson}:${optionsJson}`;
    const cached = this.renderCache.get(key);
    if (cached !== undefined) return cached;
    let svg: string;
    try {
      svg = this.wasm.render_score_svg_with_layout(this.scoreJson, this.layoutJson, optionsJson);
    } catch (cause) {
      throw this.toWorkspaceError("render", cause);
    }
    this.renderCache.set(key, svg);
    return svg;
  }

  /** Render one logical row, allowing a host to virtualize long scores. */
  renderRowSvg(rowIndex: number): string {
    this.assertLoaded();
    const optionsJson = JSON.stringify(this.options.render ?? {});
    const key = `${this.revisionNumber}:${this.layoutJson}:${rowIndex}:${optionsJson}`;
    const cached = this.renderCache.get(key);
    if (cached !== undefined) return cached;
    let svg: string;
    try {
      svg = this.wasm.render_score_svg_row(
        this.scoreJson, this.layoutJson, rowIndex, optionsJson,
      );
    } catch (cause) {
      throw this.toWorkspaceError("render", cause);
    }
    this.renderCache.set(key, svg);
    return svg;
  }

  metadata(): Record<string, unknown> {
    this.assertLoaded();
    const optionsJson = JSON.stringify(this.options.render ?? {});
    const key = `${this.revisionNumber}:${this.layoutJson}:${optionsJson}`;
    const cached = this.metadataCache.get(key);
    if (cached !== undefined) return cached;
    let metadata: Record<string, unknown>;
    try {
      metadata = JSON.parse(this.wasm.render_score_metadata(
        this.scoreJson, this.layoutJson, optionsJson,
      )) as Record<string, unknown>;
    } catch (cause) {
      throw this.toWorkspaceError("metadata", cause);
    }
    this.metadataCache.set(key, metadata);
    return metadata;
  }

  analyze(): Record<string, unknown> {
    this.assertLoaded();
    const cached = this.analysisCache.get(String(this.revisionNumber));
    if (cached !== undefined) return cached;
    let analysis: Record<string, unknown>;
    try {
      analysis = JSON.parse(this.wasm.analyze_score(this.scoreJson)) as Record<string, unknown>;
    } catch (cause) {
      throw this.toWorkspaceError("analysis", cause);
    }
    this.analysisCache.set(String(this.revisionNumber), analysis);
    return analysis;
  }

  snapshot(): WorkspaceSnapshot {
    return {
      revision: this.revisionNumber,
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

  private toWorkspaceError(operation: WorkspaceOperation, cause: unknown): AcordeWorkspaceError {
    if (cause instanceof AcordeWorkspaceError) return cause;
    const message = cause instanceof Error ? cause.message : String(cause);
    return new AcordeWorkspaceError(operation, message, cause);
  }
}
