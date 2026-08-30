# Visual regression policy

The browser fixture has checked-in Playwright screenshot baselines for Chromium, Firefox, and
WebKit. The test still checks the stable SVG hooks and dimensions separately; the screenshots
cover the final browser rasterization and accessibility presentation.

Run the review suite with:

```bash
npm install --no-save playwright@1.55.0
npx playwright install chromium firefox webkit
npx playwright test examples/browser/smoke.spec.mjs
```

To intentionally regenerate baselines, use `--update-snapshots`, inspect every changed PNG,
and run the suite again without that flag. A baseline update is valid only when the corresponding
renderer or fixture change is intentional. Keep one baseline per browser because text and SVG
antialiasing differ between engines.

The accepted difference budget is 1% of pixels with a per-pixel threshold of 0.2. This is a
small tolerance for antialiasing noise; a layout shift or missing notation should exceed it.
