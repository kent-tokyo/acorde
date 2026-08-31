/** Framework-neutral browser adapter for generated acorde WASM bindings. */

export type NoteAddress = string;

export type WorkspaceOperation =
  | "parse"
  | "layout"
  | "render"
  | "metadata"
  | "analysis"
  | "serialize"
  | "playback";

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
  parse_musicxml_report(xml: string): string;
  parse_midi(data: Uint8Array): string;
  serialize_musicxml(scoreJson: string): string;
  serialize_musicxml_report(scoreJson: string): string;
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
  analysis_cache_key(scoreJson: string): string;
  to_playback_events_ex(scoreJson: string, optionsJson: string): string;
  compute_playback_position(scoreJson: string, optionsJson: string, elapsedSecs: number): string;
  score_duration_secs(scoreJson: string): number;
}

export interface InterchangeDiagnostic {
  code: string;
  severity: string;
  source_location?: string;
  preserved_value?: string;
  loss_reason?: string;
}

export interface ImportReport {
  schema_version: number;
  format: string;
  score: Record<string, unknown>;
  diagnostics: InterchangeDiagnostic[];
}

export interface ExportReport {
  schema_version: number;
  format: string;
  output: string;
  diagnostics: InterchangeDiagnostic[];
}

export interface PlaybackEvent {
  address: NoteAddress | null;
  time_secs: number;
  duration_secs: number;
  [key: string]: unknown;
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
  selectedAddress: NoteAddress | null;
  metadata: Record<string, unknown>;
  analysis: Record<string, unknown>;
}

export interface WorkspaceHistoryState {
  revision: number;
  canUndo: boolean;
  canRedo: boolean;
}

export interface WorkspaceSelectionState {
  revision: number;
  selectedAddress: NoteAddress | null;
}

export interface WorkspaceMutationResult {
  changed: boolean;
  snapshot: WorkspaceSnapshot | null;
  history: WorkspaceHistoryState;
}

/** Encode score JSON for a Worker structured-clone boundary without a string copy. */
export function encodeScoreJson(scoreJson: string): Uint8Array {
  return new TextEncoder().encode(scoreJson);
}

/** Decode score JSON received through a Worker structured-clone boundary. */
export function decodeScoreJson(data: Uint8Array): string {
  return new TextDecoder("utf-8", { fatal: true }).decode(data);
}

export type WorkspaceRequest =
  | { id: string; type: "load-musicxml"; xml: string }
  | { id: string; type: "load-musicxml-report"; xml: string }
  | { id: string; type: "load-midi"; data: Uint8Array }
  | { id: string; type: "replace-score"; scoreJson: string }
  | { id: string; type: "replace-score-bytes"; data: Uint8Array }
  | { id: string; type: "undo" }
  | { id: string; type: "redo" }
  | { id: string; type: "history-state" }
  | { id: string; type: "select-address"; address: NoteAddress | null }
  | { id: string; type: "selection-state" }
  | { id: string; type: "snapshot" }
  | { id: string; type: "render-svg" }
  | { id: string; type: "render-row-svg"; rowIndex: number }
  | { id: string; type: "metadata" }
  | { id: string; type: "analysis" }
  | { id: string; type: "export-musicxml" }
  | { id: string; type: "export-musicxml-report" }
  | { id: string; type: "playback-events"; options?: Record<string, unknown> }
  | { id: string; type: "playback-position"; elapsedSecs: number; options?: Record<string, unknown> }
  | { id: string; type: "select-playback-at"; elapsedSecs: number; options?: Record<string, unknown> }
  | { id: string; type: "duration-seconds" };

export type WorkspaceResponse =
  | { id: string; ok: true; value: unknown }
  | {
    id: string;
    ok: false;
    error: { operation?: WorkspaceOperation; message: string };
  };

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

  /** Load MusicXML and return structured import diagnostics to the host. */
  loadMusicXmlWithReport(xml: string): ImportReport {
    let report: ImportReport;
    try {
      report = JSON.parse(this.wasm.parse_musicxml_report(xml)) as ImportReport;
    } catch (cause) {
      throw this.toWorkspaceError("parse", cause);
    }
    const prepared = this.prepareScore(JSON.stringify(report.score));
    this.undoStack.length = 0;
    this.redoStack.length = 0;
    this.installScore(prepared.scoreJson, prepared.layoutJson, prepared.layoutOptionsJson);
    this.selection.set(null);
    return report;
  }

  /** Load MIDI bytes and replace the current document transactionally. */
  loadMidi(data: Uint8Array): void {
    let nextScore: string;
    try {
      nextScore = this.wasm.parse_midi(data);
    } catch (cause) {
      throw this.toWorkspaceError("parse", cause);
    }
    const prepared = this.prepareScore(nextScore);
    this.undoStack.length = 0;
    this.redoStack.length = 0;
    this.installScore(prepared.scoreJson, prepared.layoutJson, prepared.layoutOptionsJson);
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

  /** Replace the current score from UTF-8 score JSON received via structured clone. */
  replaceScoreJsonBytes(data: Uint8Array): void {
    try {
      this.replaceScoreJson(decodeScoreJson(data));
    } catch (cause) {
      throw this.toWorkspaceError("parse", cause);
    }
  }

  /** Return the current score JSON as UTF-8 bytes for compact Worker transport. */
  scoreJsonBytes(): Uint8Array {
    this.assertLoaded();
    return encodeScoreJson(this.scoreJson);
  }

  canUndo(): boolean {
    return this.undoStack.length > 0;
  }

  canRedo(): boolean {
    return this.redoStack.length > 0;
  }

  historyState(): WorkspaceHistoryState {
    return {
      revision: this.revisionNumber,
      canUndo: this.canUndo(),
      canRedo: this.canRedo(),
    };
  }

  selectionState(): WorkspaceSelectionState {
    return {
      revision: this.revisionNumber,
      selectedAddress: this.selection.get(),
    };
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
    let cacheKey: string;
    try {
      cacheKey = this.wasm.analysis_cache_key(this.scoreJson);
    } catch (cause) {
      throw this.toWorkspaceError("analysis", cause);
    }
    const cached = this.analysisCache.get(cacheKey);
    if (cached !== undefined) return cached;
    let analysis: Record<string, unknown>;
    try {
      analysis = JSON.parse(this.wasm.analyze_score(this.scoreJson)) as Record<string, unknown>;
    } catch (cause) {
      throw this.toWorkspaceError("analysis", cause);
    }
    this.analysisCache.set(cacheKey, analysis);
    return analysis;
  }

  /** Serialize the loaded score for an offline MusicXML export. */
  exportMusicXml(): string {
    this.assertLoaded();
    try {
      return this.wasm.serialize_musicxml(this.scoreJson);
    } catch (cause) {
      throw this.toWorkspaceError("serialize", cause);
    }
  }

  /** Serialize MusicXML and return structured export diagnostics to the host. */
  exportMusicXmlWithReport(): ExportReport {
    this.assertLoaded();
    try {
      return JSON.parse(this.wasm.serialize_musicxml_report(this.scoreJson)) as ExportReport;
    } catch (cause) {
      throw this.toWorkspaceError("serialize", cause);
    }
  }

  /** Produce host-independent playback events for the loaded score. */
  playbackEvents(options: Record<string, unknown> = {}): PlaybackEvent[] {
    this.assertLoaded();
    try {
      return JSON.parse(this.wasm.to_playback_events_ex(
        this.scoreJson,
        JSON.stringify(options),
      )) as PlaybackEvent[];
    } catch (cause) {
      throw this.toWorkspaceError("playback", cause);
    }
  }

  /** Synchronize notation selection with a sounding event from the host audio scheduler. */
  selectPlaybackEvent(event: Pick<PlaybackEvent, "address">): void {
    this.selection.set(event.address);
  }

  /** Find the sounding event at an elapsed time, ignoring metronome events. */
  playbackEventAt(
    elapsedSecs: number,
    options: Record<string, unknown> = {},
  ): PlaybackEvent | null {
    if (!Number.isFinite(elapsedSecs) || elapsedSecs < 0) return null;
    const active = this.playbackEvents(options).filter((event) =>
      event.address !== null
      && event.time_secs <= elapsedSecs
      && elapsedSecs < event.time_secs + event.duration_secs,
    );
    return active[active.length - 1] ?? null;
  }

  /** Select the sounding event at an elapsed time and return it for host cursor updates. */
  selectPlaybackAt(
    elapsedSecs: number,
    options: Record<string, unknown> = {},
  ): PlaybackEvent | null {
    const event = this.playbackEventAt(elapsedSecs, options);
    this.selectPlaybackEvent(event ?? { address: null });
    return event;
  }

  /** Resolve the playback cursor at an elapsed time in seconds. */
  playbackPosition(
    elapsedSecs: number,
    options: Record<string, unknown> = {},
  ): Record<string, unknown> | null {
    this.assertLoaded();
    try {
      return JSON.parse(this.wasm.compute_playback_position(
        this.scoreJson,
        JSON.stringify(options),
        elapsedSecs,
      )) as Record<string, unknown> | null;
    } catch (cause) {
      throw this.toWorkspaceError("playback", cause);
    }
  }

  /** Return the score duration used to schedule host playback. */
  durationSeconds(): number {
    this.assertLoaded();
    try {
      return this.wasm.score_duration_secs(this.scoreJson);
    } catch (cause) {
      throw this.toWorkspaceError("playback", cause);
    }
  }

  snapshot(): WorkspaceSnapshot {
    return {
      revision: this.revisionNumber,
      scoreJson: this.scoreJson,
      layoutJson: this.layoutJson,
      selectedAddress: this.selection.get(),
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

/**
 * Handle one serializable workspace message. A host can call this from a Worker
 * `onmessage` handler and post the returned response back to the UI thread.
 */
export function handleWorkspaceRequest(
  workspace: AcordeWorkspace,
  request: WorkspaceRequest,
): WorkspaceResponse {
  try {
    switch (request.type) {
      case "load-musicxml":
        workspace.loadMusicXml(request.xml);
        return { id: request.id, ok: true, value: workspace.snapshot() };
      case "load-musicxml-report": {
        const report = workspace.loadMusicXmlWithReport(request.xml);
        return { id: request.id, ok: true, value: { report, snapshot: workspace.snapshot() } };
      }
      case "load-midi":
        workspace.loadMidi(request.data);
        return { id: request.id, ok: true, value: workspace.snapshot() };
      case "replace-score":
        workspace.replaceScoreJson(request.scoreJson);
        return { id: request.id, ok: true, value: workspace.snapshot() };
      case "replace-score-bytes":
        workspace.replaceScoreJsonBytes(request.data);
        return { id: request.id, ok: true, value: workspace.snapshot() };
      case "undo":
      case "redo": {
        const changed = request.type === "undo" ? workspace.undo() : workspace.redo();
        return {
          id: request.id,
          ok: true,
          value: {
            changed,
            snapshot: changed ? workspace.snapshot() : null,
            history: workspace.historyState(),
          } as WorkspaceMutationResult,
        };
      }
      case "history-state":
        return { id: request.id, ok: true, value: workspace.historyState() };
      case "select-address":
        workspace.selection.set(request.address);
        return { id: request.id, ok: true, value: workspace.selectionState() };
      case "selection-state":
        return { id: request.id, ok: true, value: workspace.selectionState() };
      case "snapshot":
        return { id: request.id, ok: true, value: workspace.snapshot() };
      case "render-svg":
        return { id: request.id, ok: true, value: workspace.renderSvg() };
      case "render-row-svg":
        return { id: request.id, ok: true, value: workspace.renderRowSvg(request.rowIndex) };
      case "metadata":
        return { id: request.id, ok: true, value: workspace.metadata() };
      case "analysis":
        return { id: request.id, ok: true, value: workspace.analyze() };
      case "export-musicxml":
        return { id: request.id, ok: true, value: workspace.exportMusicXml() };
      case "export-musicxml-report":
        return { id: request.id, ok: true, value: workspace.exportMusicXmlWithReport() };
      case "playback-events":
        return { id: request.id, ok: true, value: workspace.playbackEvents(request.options) };
      case "playback-position":
        return {
          id: request.id,
          ok: true,
          value: workspace.playbackPosition(request.elapsedSecs, request.options),
        };
      case "select-playback-at":
        return {
          id: request.id,
          ok: true,
          value: workspace.selectPlaybackAt(request.elapsedSecs, request.options),
        };
      case "duration-seconds":
        return { id: request.id, ok: true, value: workspace.durationSeconds() };
      default:
        return {
          id: request.id,
          ok: false,
          error: { message: "unsupported workspace request" },
        };
    }
  } catch (cause) {
    if (cause instanceof AcordeWorkspaceError) {
      return {
        id: request.id,
        ok: false,
        error: { operation: cause.operation, message: cause.message },
      };
    }
    return {
      id: request.id,
      ok: false,
      error: { message: cause instanceof Error ? cause.message : String(cause) },
    };
  }
}
