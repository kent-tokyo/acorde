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
