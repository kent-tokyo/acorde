# Print SVG contract

This document defines the host-neutral requirements for exporting an `acorde-layout`
page as SVG. It deliberately stops at deterministic page geometry; PDF conversion,
font installation or embedding, file I/O, and OS printing belong to the consuming host.

## Required page representation

- Use physical millimetres for page coordinates and dimensions.
- Set `width` and `height` to the page's physical size and use a matching `viewBox`.
- Emit one independent SVG artifact per logical page. The page address and logical page
  number come from `PrintLayoutResult`, not from a viewport or DOM position.
- Keep measure and cross-system span ownership from the page artifact unchanged. An SVG
  exporter must not recompute physical measure indices.
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
- No caller-provided SVG/XML fragments are accepted as annotations.

## Metadata and colors

Every page should expose stable metadata sufficient for reproducibility: contract version,
score fingerprint, renderer version, page address, logical page number, and the selected
glyph-resource policy. Colors are opaque sRGB values unless a host explicitly documents a
different profile; transparency and crop marks must follow `PrintConfig`.

## Verification

For a fixed score, `PrintConfig`, renderer version, and resource contract, page geometry and
serialized metadata must be byte-deterministic. Hosts should separately verify PDF boxes,
font embedding/substitution, raster output, and printer behavior; those checks are not
evidence that the core renderer owns those capabilities.
