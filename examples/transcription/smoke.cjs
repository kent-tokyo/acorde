// Run against the local server with Playwright installed and CHROME set if needed.
const assert = require("node:assert/strict");
const { createHash } = require("node:crypto");
const { chromium } = require("playwright");

async function main() {
  const browser = await chromium.launch({
    headless: true,
    ...(process.env.CHROME ? { executablePath: process.env.CHROME } : {}),
  });
  try {
    const page = await browser.newPage();
    const errors = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await page.goto(process.env.TRANSCRIPTION_URL || "http://127.0.0.1:8765/");
    const project = {
      version: 1,
      tempo: 120,
      numerator: 4,
      denominator: 4,
      barStart: 0,
      instrument: "accordion",
      notes: [
        { start: 0.05, end: 2.97, pitch: 56, hand: "right" },
        { start: 0.06, end: 2.97, pitch: 52, hand: "right" },
        { start: 0.18, end: 2.97, pitch: 62, hand: "right" },
      ],
    };
    await page.locator("#openProject").setInputFiles({
      name: "session.json",
      mimeType: "application/json",
      buffer: Buffer.from(JSON.stringify(project)),
    });
    await page.locator("#rangeLeft").click();
    await page.locator("#chords").click();
    await page.getByText("E7（5度省略）", { exact: false }).waitFor();
    let downloadPromise = page.waitForEvent("download");
    await page.locator("#musicxml").click();
    const score = await (await downloadPromise).createReadStream();
    let xml = "";
    for await (const chunk of score) xml += chunk;
    assert.match(xml, /<part-name>Accordion<\/part-name>/);
    const wav = Buffer.alloc(44 + 22050 * 4 * 2);
    wav.write("RIFF", 0);
    wav.writeUInt32LE(wav.length - 8, 4);
    wav.write("WAVEfmt ", 8);
    wav.writeUInt32LE(16, 16);
    wav.writeUInt16LE(1, 20);
    wav.writeUInt16LE(1, 22);
    wav.writeUInt32LE(22050, 24);
    wav.writeUInt32LE(44100, 28);
    wav.writeUInt16LE(2, 32);
    wav.writeUInt16LE(16, 34);
    wav.write("data", 36);
    wav.writeUInt32LE(wav.length - 44, 40);
    await page
      .locator("#file")
      .setInputFiles({ name: "audio.wav", mimeType: "audio/wav", buffer: wav });
    await page.waitForFunction(
      () => document.getElementById("audio").duration === 4,
    );
    await page.locator("#preview").click();
    await page.locator("#scoreImage").waitFor({ state: "visible" });
    await page.locator("#measure").fill("2");
    await page.locator("#measureLoop").click();
    assert.equal(await page.locator("#loopStart").inputValue(), "2.000");
    assert.equal(await page.locator("#loopEnd").inputValue(), "4.000");
    await page.locator("#loopStart").fill("0");
    await page.locator("#loopEnd").fill("4");
    await page.locator("#rate").selectOption("0.5");
    await page.locator("#loop").click();
    await page.waitForFunction(() => !document.getElementById("audio").paused);
    const playback = await page.locator("#audio").evaluate((audio) => ({
      rate: audio.playbackRate,
      preservesPitch: audio.preservesPitch,
    }));
    assert.deepEqual(playback, { rate: 0.5, preservesPitch: true });
    await page.locator("#stop").click();
    assert.equal(
      await page.locator("#audio").evaluate((audio) => audio.paused),
      true,
    );
    await page.locator("#low").fill("60");
    await page.locator("#high").fill("70");
    await page.route("**/transcribe", async (route) => {
      const headers = route.request().headers();
      assert.equal(headers["x-pitch-low"], "60");
      assert.equal(headers["x-pitch-high"], "70");
      await route.fulfill({
        json: {
          notes: [{ start: 0.2, end: 1, pitch: 64, hand: "right" }],
          context_limited_notes: 1,
        },
      });
    });
    await page.locator("#analyze").click();
    await page.getByText("1音の候補を作りました。", { exact: false }).waitFor();
    await page
      .getByText("1音は解析範囲の端まで続いています。", { exact: false })
      .waitFor();
    assert.equal(await page.locator("#scoreImage").isHidden(), true);
    downloadPromise = page.waitForEvent("download");
    await page.locator("#saveProject").click();
    const session = await (await downloadPromise).createReadStream();
    let saved = "";
    for await (const chunk of session) saved += chunk;
    const pitches = JSON.parse(saved)
      .notes.map((note) => note.pitch)
      .sort((a, b) => a - b);
    assert.deepEqual(pitches, [52, 56, 64]);
    await page.locator("#undo").click();
    downloadPromise = page.waitForEvent("download");
    await page.locator("#saveProject").click();
    const undoSession = await (await downloadPromise).createReadStream();
    let restored = "";
    for await (const chunk of undoSession) restored += chunk;
    assert.deepEqual(
      JSON.parse(restored)
        .notes.map((note) => note.pitch)
        .sort((a, b) => a - b),
      [52, 56, 62],
    );
    await page.unroute("**/transcribe");
    await page.route("**/transcribe", (route) => route.fulfill({
      json: {
        notes: [{ start: 0.3, end: 1, pitch: 64, hand: "right" }],
        boundary_notes: [{ start: 0.19, end: 1, pitch: 62, hand: "right", context_limited: true }],
      },
    }));
    await page.locator("#loopStart").fill("0.2");
    await page.locator("#analyze").click();
    await page.locator("#boundaryPanel").getByText("近い音符あり", { exact: false }).waitFor();
    await page.locator("#boundaryPanel").getByText("音の長さを確認", { exact: false }).waitFor();
    await page.locator("#boundaryPanel button").click();
    assert.equal(await page.locator("#boundaryPanel button").innerText(), "追加済み");
    downloadPromise = page.waitForEvent("download");
    await page.locator("#saveProject").click();
    let boundarySaved = "";
    for await (const chunk of await (await downloadPromise).createReadStream()) boundarySaved += chunk;
    assert.equal(JSON.parse(boundarySaved).notes.filter((note) => note.pitch === 62).length, 2);
    await page.locator("#undo").click();
    assert.equal(await page.locator("#boundaryPanel button").innerText(), "追加");
    downloadPromise = page.waitForEvent("download");
    await page.locator("#saveProject").click();
    let boundaryUndone = "";
    for await (const chunk of await (await downloadPromise).createReadStream()) boundaryUndone += chunk;
    assert.equal(JSON.parse(boundaryUndone).notes.filter((note) => note.pitch === 62).length, 1);
    await page.unroute("**/transcribe");
    await page.route("**/transcribe", (route) => route.fulfill({
      json: { notes: [{ start: 0.2, end: 1, pitch: 64, hand: "right" }] },
    }));
    const pickupProject = {
      version: 1,
      tempo: 120,
      numerator: 4,
      denominator: 4,
      barStart: 1,
      pickupSteps: 4,
      notes: [{ start: 0.75, end: 1.25, pitch: 60, hand: "right" }],
    };
    await page.locator("#openProject").setInputFiles({
      name: "pickup.json",
      mimeType: "application/json",
      buffer: Buffer.from(JSON.stringify(pickupProject)),
    });
    await page.locator("#preview").click();
    await page.locator("#scoreImage").waitFor({ state: "visible" });
    await page.locator("#measure").fill("1");
    await page.locator("#measureLoop").click();
    assert.equal(await page.locator("#loopStart").inputValue(), "0.500");
    assert.equal(await page.locator("#loopEnd").inputValue(), "1.000");
    await page.locator("#measure").fill("2");
    await page.locator("#measureLoop").click();
    assert.equal(await page.locator("#loopStart").inputValue(), "1.000");
    assert.equal(await page.locator("#loopEnd").inputValue(), "3.000");
    downloadPromise = page.waitForEvent("download");
    await page.locator("#musicxml").click();
    const pickupStream = await (await downloadPromise).createReadStream();
    let pickupXml = "";
    for await (const chunk of pickupStream) pickupXml += chunk;
    assert.match(pickupXml, /<measure number="1" implicit="yes">/);
    assert.match(pickupXml, /<tie type="start"\/>/);
    assert.match(pickupXml, /<tie type="stop"\/>/);
    const recordingHash = createHash("sha256").update(wav).digest("hex");
    const freshPage = await browser.newPage();
    await freshPage.goto(
      process.env.TRANSCRIPTION_URL || "http://127.0.0.1:8765/",
    );
    await freshPage
      .locator("#file")
      .setInputFiles({ name: "fresh.wav", mimeType: "audio/wav", buffer: wav });
    await freshPage.getByText("録音が編集内容と一致しています。").waitFor();
    const freshDownload = freshPage.waitForEvent("download");
    await freshPage.locator("#saveProject").click();
    let freshSaved = "";
    for await (const chunk of await (await freshDownload).createReadStream())
      freshSaved += chunk;
    assert.equal(JSON.parse(freshSaved).recordingSha256, recordingHash);
    await freshPage.close();
    await page.locator("#openProject").setInputFiles({
      name: "matched.json",
      mimeType: "application/json",
      buffer: Buffer.from(
        JSON.stringify({ ...pickupProject, recordingSha256: recordingHash }),
      ),
    });
    await page.getByText("録音が編集内容と一致しています。").waitFor();
    const altered = Buffer.from(wav);
    altered[altered.length - 1] = 1;
    await page.locator("#file").setInputFiles({
      name: "different.wav",
      mimeType: "audio/wav",
      buffer: altered,
    });
    await page.getByText(/選択した録音は編集内容の録音と異なります/).waitFor();
    downloadPromise = page.waitForEvent("download");
    await page.locator("#saveProject").click();
    let mismatchSave = "";
    for await (const chunk of await (await downloadPromise).createReadStream())
      mismatchSave += chunk;
    assert.equal(JSON.parse(mismatchSave).recordingSha256, recordingHash);
    await page.locator("#analyze").click();
    await page.getByText("1音の候補を作りました。", { exact: false }).waitFor();
    await page
      .getByText("編集内容に元の録音の照合情報がありません。")
      .waitFor();
    await page.locator("#undo").click();
    await page.getByText(/選択した録音は編集内容の録音と異なります/).waitFor();
    assert.deepEqual(errors, []);
    console.log(
      "Session restore, chord candidate, MusicXML, and range editing passed.",
    );
  } finally {
    await browser.close();
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
