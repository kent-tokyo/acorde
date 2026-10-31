import { test, expect } from "playwright/test";

test.beforeEach(async ({ page }) => {
  // Headless browsers do not share a Web Audio clock contract: Firefox can
  // suspend or finish an output graph before Playwright observes the UI state.
  // This smoke test exercises the host scheduler and transport UI, not a
  // physical output device, so use a monotonic in-page Web Audio substitute.
  await page.addInitScript(() => {
    class FakeAudioParam {
      setValueAtTime() {}
      exponentialRampToValueAtTime() {}
    }

    class FakeOscillator extends EventTarget {
      constructor(context) {
        super();
        this.context = context;
        this.frequency = new FakeAudioParam();
        this.type = "sine";
      }

      connect() { return this; }
      start() {}
      stop(at = this.context.currentTime) {
        const delay = Math.max(0, at - this.context.currentTime) * 1000;
        setTimeout(() => this.dispatchEvent(new Event("ended")), delay);
      }
    }

    class FakeGain {
      constructor() { this.gain = new FakeAudioParam(); }
      connect() { return this; }
    }

    class FakeAudioContext {
      constructor() {
        this.startedAt = performance.now();
        this.destination = {};
        this.state = "running";
      }

      get currentTime() { return (performance.now() - this.startedAt) / 1000; }
      async resume() { this.state = "running"; }
      createOscillator() { return new FakeOscillator(this); }
      createGain() { return new FakeGain(); }
    }

    Object.defineProperty(window, "AudioContext", {
      configurable: true,
      writable: true,
      value: FakeAudioContext,
    });
  });
});

test("browser chord plans preserve source timing and replay a chosen proposal", async ({ page }) => {
  await page.goto("http://127.0.0.1:8000/examples/browser/");
  const result = await page.evaluate(async () => {
    const wasm = await import("/crates/wasm/pkg/acorde_wasm.js");
    await wasm.default();
    const abc = 'X:1\nT:Two chord changes\nM:4/4\nL:1/4\nK:C\n"C"C E "G7"G B|"C"c4|';
    const score = wasm.parse_abc(abc);
    const extracted = JSON.parse(wasm.extract_chord_plan(score, "{}"));
    const proposals = JSON.parse(wasm.suggest_chord_plans(score, JSON.stringify({
      source_plan: extracted.plan,
    })));
    const provided = proposals.candidates.find((candidate) => candidate.id === "provided-v1");
    const replay = JSON.parse(wasm.arrange_from_chord_proposal(
      score, JSON.stringify(provided.resolved), "held-chord",
    ));
    const arranged = JSON.stringify(replay.arrangement.score);
    const restored = JSON.parse(wasm.parse_mscz(wasm.serialize_mscz(arranged)));
    return {
      offsets: extracted.plan.entries.map((entry) => [entry.position.measure_index, entry.offset_beats]),
      origins: provided.plan.entries.map((entry) => entry.origin),
      diagnostics: replay.diagnostics,
      scoreReturned: Boolean(replay.arrangement.score),
      rendered: wasm.render_score_svg(arranged, "{}").includes("<svg"),
      played: JSON.parse(wasm.to_playback_events_ex(arranged, "{}")).length > 0,
      exported: restored.parts[0].staves[0].measures[0].voices[0][0].pitches[0].step
        === replay.arrangement.score.parts[0].staves[0].measures[0].voices[0][0].pitches[0].step,
    };
  });
  expect(result.offsets).toEqual([[0, 0], [0, 2], [1, 0]]);
  expect(result.origins).toEqual(["written-symbol", "written-symbol", "written-symbol"]);
  expect(result.diagnostics).toEqual([]);
  expect(result.scoreReturned).toBe(true);
  expect(result.rendered).toBe(true);
  expect(result.played).toBe(true);
  expect(result.exported).toBe(true);
});

test("reparsed ABC replays a preset and redundant chords keep accompaniment phase", async ({ page }) => {
  await page.goto("http://127.0.0.1:8000/examples/browser/");
  const result = await page.evaluate(async () => {
    const wasm = await import("/crates/wasm/pkg/acorde_wasm.js");
    await wasm.default();
    const abc = "X:1\nM:4/4\nL:1/4\nQ:1/4=60\nK:C\nC E G c|";
    const first = wasm.parse_abc(abc);
    const second = wasm.parse_abc(abc);
    const chord = (offset_beats) => ({
      measure_index: 0, offset_beats,
      chord: { root: "C", kind: "major", bass: null, degrees: [] },
    });
    const direct = (chords) => JSON.parse(wasm.arrange_with_explicit_chords(first,
      JSON.stringify({ pattern: "alternating-bass-chord", chords })));
    const single = direct([chord(0)]);
    const repeated = direct([chord(0), chord(2)]);
    const left = (score) => score.parts[0].staves[1].measures[0].voices[0]
      .map((note) => [note.pitches.map((pitch) => `${pitch.step}${pitch.octave}`), note.duration]);
    const selection = {
      chords: [chord(0), chord(2)], alternative_chords: null,
      inversion: null, bass_mode: "free-bass",
    };
    const selected = JSON.parse(wasm.arrange_accordion_preset(first,
      JSON.stringify(selection), "alternating-bass"));
    const replayed = JSON.parse(wasm.arrange_from_resolved_preset(second,
      JSON.stringify(selected.resolved_config)));
    const changed = JSON.parse(wasm.arrange_from_resolved_preset(
      wasm.parse_abc(abc.replace("C E G c", "C E A c")),
      JSON.stringify(selected.resolved_config)));
    const arrangedJson = JSON.stringify(replayed.score);
    const restored = JSON.parse(wasm.parse_mscz(wasm.serialize_mscz(arrangedJson)));
    const events = (score) => JSON.parse(wasm.to_playback_events_ex(JSON.stringify(score), "{}"))
      .map((event) => [event.pitch_midi, event.time_beats, event.duration_beats]);
    return {
      idsDiffer: JSON.parse(first).parts[0].staves[0].measures[0].voices[0][0].id
        !== JSON.parse(second).parts[0].staves[0].measures[0].voices[0][0].id,
      fingerprintVersion: selected.resolved_config.input_fingerprint.startsWith("v2:"),
      samePattern: JSON.stringify(left(single.score)) === JSON.stringify(left(repeated.score)),
      samePlayback: JSON.stringify(events(single.score)) === JSON.stringify(events(repeated.score)),
      thirdBass: left(repeated.score)[2][0],
      replayDiagnostics: replayed.diagnostics,
      sourceIdMatches: replayed.score.parts[0].staves[0].measures[0].voices[0][0].id
        === JSON.parse(second).parts[0].staves[0].measures[0].voices[0][0].id,
      playbackMatches: JSON.stringify(events(selected.score)) === JSON.stringify(events(replayed.score)),
      changedCode: changed.diagnostics[0].code,
      rendered: wasm.render_score_svg(arrangedJson, "{}").includes("<svg"),
      exported: restored.parts[0].staves[0].measures[0].voices[0][0].pitches[0].step
        === replayed.score.parts[0].staves[0].measures[0].voices[0][0].pitches[0].step,
    };
  });
  expect(result.idsDiffer).toBe(true);
  expect(result.fingerprintVersion).toBe(true);
  expect(result.samePattern).toBe(true);
  expect(result.samePlayback).toBe(true);
  expect(result.thirdBass).toEqual(["G2"]);
  expect(result.replayDiagnostics).toEqual([]);
  expect(result.sourceIdMatches).toBe(true);
  expect(result.playbackMatches).toBe(true);
  expect(result.changedCode).toBe("input-mismatch");
  expect(result.rendered).toBe(true);
  expect(result.exported).toBe(true);
});

test("browser contract renders and exposes selectable notes", async ({ page }) => {
  await page.goto("http://127.0.0.1:8000/examples/browser/");
  const status = page.locator("#status");
  await expect(status).toHaveText(/PASS:/);
  await expect(page.locator("#apply")).toBeEnabled();
  await expect(page.locator("#analyze")).toBeEnabled();
  await expect(page.locator("#export")).toBeEnabled();
  await expect(page.getByRole("button", { name: "Play score" })).toBeEnabled();
  const tempo = page.getByRole("slider", { name: "Playback tempo multiplier percentage" });
  await expect(tempo).toHaveValue("100");
  await tempo.evaluate((element) => {
    element.value = "80";
    element.dispatchEvent(new Event("input", { bubbles: true }));
  });
  await expect(page.locator("#tempo-value")).toHaveText("80%");
  await page.getByRole("checkbox", { name: "Loop measure range" }).check();
  await expect(page.getByRole("spinbutton", { name: "Loop start measure" })).toHaveValue("1");
  const loopEnd = page.getByRole("spinbutton", { name: "Loop end measure" });
  await expect(loopEnd).toHaveValue("1");
  await loopEnd.fill("3");
  await loopEnd.dispatchEvent("change");
  await expect(loopEnd).toHaveValue("3");
  await page.getByRole("checkbox", { name: "Metronome" }).check();
  await page.getByRole("button", { name: "Play score" }).click();
  await expect(page.locator("#transport-status")).toHaveText("Playing");
  await expect(page.locator("#transport-position")).toContainText("M1 B1");
  await expect(page.getByRole("button", { name: "Rewind" })).toBeEnabled();
  await page.getByRole("button", { name: "Rewind" }).click();
  await expect(page.locator("#transport-position")).toContainText("0:00 · M1 B1");
  await expect(page.getByRole("button", { name: "Pause" })).toBeEnabled();
  await page.getByRole("button", { name: "Pause" }).click();
  await expect(page.locator("#transport-status")).toHaveText("Paused");
  const seek = page.getByRole("slider", { name: "Seek position in seconds" });
  await expect(seek).toBeEnabled();
  await seek.evaluate((element) => {
    element.value = "0.75";
    element.dispatchEvent(new Event("change", { bubbles: true }));
  });
  await expect(page.locator("#transport-position")).toContainText("M1 B2");
  await expect(page.getByLabel("Playback scheduling trace")).toContainText('"kind":"seek"');
  await expect(page.getByLabel("Playback scheduling trace")).toContainText('"kind":"schedule"');
  await page.getByRole("button", { name: "Play score" }).click();
  await expect(page.locator("#transport-status")).toHaveText("Playing");
  await page.getByRole("button", { name: "Stop" }).click();
  await expect(page.locator("#transport-status")).toHaveText("Stopped");
  const countIn = page.getByRole("spinbutton", { name: "Count-in beats" });
  await countIn.fill("1");
  await page.locator("#practice-host").focus();
  await page.keyboard.press("Space");
  await expect(page.locator("#transport-status")).toHaveText("Counting-in");
  await expect(page.getByLabel("Playback scheduling trace")).toContainText('"kind":"count-in"');
  await page.keyboard.press("Space");
  await expect(page.locator("#transport-status")).toHaveText("Paused");
  await page.getByRole("button", { name: "Analyze" }).click();
  await expect(page.locator("#analysis")).not.toHaveText("");
  await expect(page.locator("#analysis")).toContainText('"schema_version": 13');
  await expect(page.locator("#analysis")).toContainText('"key_estimates"');
  await expect(page.locator("#analysis")).toContainText('"weighted_covered_beats"');
  await expect(page.locator("#metadata")).toContainText('"tablature_staves"');
  // Keep this browser boundary pinned to the current SVG metadata contract.
  // A contract bump requires an explicit browser-review update rather than
  // silently accepting a different payload.
  await expect(page.locator("#metadata")).toContainText('"contract_version": 22');
  await expect(page.locator("#metadata")).toContainText('"note_semantics"');
  await expect(page.locator("#metadata")).toContainText('"duration_beats"');
  await expect(page.locator("#metadata")).toContainText('"pitch_midi_cents"');
  await expect(page.locator("#metadata")).toContainText('"articulations"');
  await expect(page.locator("[data-note-addr]")).toHaveCount(10);
  const source = page.locator("#source");
  await expect(source).toHaveValue(/<score-partwise/);
  await source.fill((await source.inputValue()).replace("<step>C</step>", "<step>D</step>"));
  await page.getByRole("button", { name: "Apply edited MusicXML" }).click();
  await expect(page.getByRole("button", { name: "Undo" })).toBeEnabled();
  await page.getByRole("button", { name: "Undo" }).click();
  await expect(page.getByRole("button", { name: "Redo" })).toBeEnabled();
  await page.getByRole("button", { name: "Redo" }).click();
  await page.locator("[data-note-addr]").first().focus();
  await page.keyboard.press("Enter");
  await expect(status).toContainText("selected 0:0:0:0:0");
  await expect(page.locator("#score svg")).toHaveAttribute("role", "img");
  const svg = page.locator("#score svg");
  const box = await svg.boundingBox();
  expect(box?.width).toBeGreaterThan(0);
  expect(box?.height).toBeGreaterThan(0);
  await expect(page.locator("#score svg")).toHaveScreenshot("score.png", {
    fullPage: true,
    animations: "disabled",
    caret: "hide",
    // Browser engines and runner images can rasterize the same deterministic SVG differently.
    // Structural SVG and renderer golden tests remain exact; this browser smoke threshold catches
    // material host-facing regressions without failing on known cross-engine antialiasing drift.
    maxDiffPixelRatio: 0.03,
    threshold: 0.2,
  });
  await expect(page.locator("#score")).toHaveAttribute("aria-describedby", "score-description");
  await expect(page.locator("#score-description")).not.toHaveText("");
  await page.screenshot({ path: test.info().outputPath(`${test.info().project.name}.png`), fullPage: true });
});
