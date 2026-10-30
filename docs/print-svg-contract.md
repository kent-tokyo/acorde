# Print SVG contract

`acorde-render-svg::render_print_page_svgs(score, layout)` turns a validated
`PrintLayoutResult` into one physical SVG per page. The WASM viewer export has the
same name and returns a JSON array of SVG strings. `acorde-pdf::export_score_print_pdf`
renders those pages and assembles a vector PDF. Font installation, file I/O, and
OS printing belong to the consuming host.

## Required page representation

- Use physical millimetres for page coordinates and dimensions.
- Set `width` and `height` to the page's physical size and use a matching `viewBox`.
- Emit one independent SVG artifact per logical page. The page address and logical page
  number come from `PrintLayoutResult`, not from a viewport or DOM position.
- Keep measure and cross-system span ownership from the page artifact unchanged. The renderer
  uses the supplied system breaks and physical top positions; it does not repaginate.
- Preserve deterministic ordering: page metadata, definitions/resources, systems, then
  publication text and diagnostics.

## Resources and text

- Built-in vector glyphs may be embedded by the renderer and are identified by their
  stable resource key.
- Host-provided resources must be declared through `GlyphResourcePolicy`; an unresolved
  resource is a blocking diagnostic, never a blank glyph or silent font substitution.
- Host glyph metrics must be validated before placement. Non-finite geometry, negative
  extents, overflow, and page overflow remain blocking diagnostics.
- Publication text is escaped SVG text with its typed style, alignment, line-box height,
  and physical placement. Hosts may convert it to paths only after recording the selected
  font/resource contract.
- Lyrics, dynamics, chord symbols, articulations, beams, and supported span continuations
  come from the shared score renderer. The same font-independent collision estimates are
  used in interactive and print SVGs. The text font supplied to the PDF backend can still
  differ from those estimates.
- Built-in print export rejects host glyphs, image resources, frames, preserved colors,
  crop marks, and unpositioned score text explicitly.
- No caller-provided SVG/XML fragments are accepted as annotations.

## Metadata and colors

Page order, physical size, title, and page number come from the validated layout.
The SVG currently does not embed a score fingerprint or renderer version; hosts that
archive pages should record those alongside the SVG. Print output is monochrome.

## Verification

For a fixed score, `PrintConfig`, renderer version, and resource contract, page SVGs
are byte-deterministic. Tests cover A4 and Letter PDF media boxes and the browser-facing
WASM API. Hosts should separately verify font embedding/substitution, raster output,
and printer behavior.
