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
  private readonly undoStack: string[] = [];
  private readonly redoStack: string[] = [];

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
    this.undoStack.length = 0;
    this.redoStack.length = 0;
    this.installScore(nextScore, nextLayout, layoutOptionsJson);
    this.selection.set(null);
  }

  /** Replace the current score with an already serialized score and record it for undo. */
  replaceScoreJson(nextScoreJson: string): void {
    const layoutOptionsJson = JSON.stringify(this.options.layout ?? {});
    let nextLayout: string;
    try {
      nextLayout = this.wasm.compute_layout_ex(nextScoreJson, layoutOptionsJson);
    } catch (cause) {
      throw this.toWorkspaceError("layout", cause);
    }
    if (this.scoreJson) this.undoStack.push(this.scoreJson);
    this.redoStack.length = 0;
    this.installScore(nextScoreJson, nextLayout, layoutOptionsJson);
    this.selection.set(null);
  }

  canUndo(): boolean {
    return this.undoStack.length > 0;
  }

  canRedo(): boolean {
    return this.redoStack.length > 0;
  }

  undo(): boolean {
    const previous = this.undoStack[this.undoStack.length - 1];
    if (previous === undefined) return false;
    const restored = this.prepareScore(previous);
    this.undoStack.pop();
    this.redoStack.push(this.scoreJson);
    this.installScore(restored.scoreJson, restored.layoutJson, restored.layoutOptionsJson);
    this.selection.set(null);
    return true;
  }

  redo(): boolean {
    const next = this.redoStack[this.redoStack.length - 1];
    if (next === undefined) return false;
    const restored = this.prepareScore(next);
    this.redoStack.pop();
    this.undoStack.push(this.scoreJson);
    this.installScore(restored.scoreJson, restored.layoutJson, restored.layoutOptionsJson);
    this.selection.set(null);
    return true;
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

  private prepareScore(scoreJson: string): {
    scoreJson: string;
    layoutJson: string;
    layoutOptionsJson: string;
  } {
    const layoutOptionsJson = JSON.stringify(this.options.layout ?? {});
    try {
      return {
        scoreJson,
        layoutJson: this.wasm.compute_layout_ex(scoreJson, layoutOptionsJson),
        layoutOptionsJson,
      };
    } catch (cause) {
      throw this.toWorkspaceError("layout", cause);
    }
  }

  private installScore(scoreJson: string, layoutJson: string, layoutOptionsJson: string): void {
    this.layoutCache.clear();
    this.renderCache.clear();
    this.metadataCache.clear();
    this.analysisCache.clear();
    this.revisionNumber += 1;
    this.scoreJson = scoreJson;
    this.layoutJson = layoutJson;
    this.layoutCache.set(`${this.revisionNumber}:${layoutOptionsJson}`, layoutJson);
  }

  private toWorkspaceError(operation: WorkspaceOperation, cause: unknown): AcordeWorkspaceError {
    if (cause instanceof AcordeWorkspaceError) return cause;
    const message = cause instanceof Error ? cause.message : String(cause);
    return new AcordeWorkspaceError(operation, message, cause);
  }
}
