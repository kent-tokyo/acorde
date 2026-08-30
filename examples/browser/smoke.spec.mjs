import { test, expect } from "playwright/test";

test("browser contract renders and exposes selectable notes", async ({ page }) => {
  await page.goto("http://127.0.0.1:8000/examples/browser/");
  const status = page.locator("#status");
  await expect(status).toHaveText(/PASS:/);
  await expect(page.locator("[data-note-addr]")).toHaveCount(4);
  await page.locator("[data-note-addr]").first().focus();
  await page.keyboard.press("Enter");
  await expect(status).toContainText("selected 0:0:0:0:0");
  await expect(page.locator("#score svg")).toHaveAttribute("role", "img");
  const svg = page.locator("#score svg");
  const box = await svg.boundingBox();
  expect(box?.width).toBeGreaterThan(0);
  expect(box?.height).toBeGreaterThan(0);
  await expect(page).toHaveScreenshot("score.png", {
    fullPage: true,
    animations: "disabled",
    caret: "hide",
    maxDiffPixelRatio: 0.01,
    threshold: 0.2,
  });
  await page.screenshot({ path: test.info().outputPath(`${test.info().project.name}.png`), fullPage: true });
});
