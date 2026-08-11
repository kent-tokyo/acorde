# docs/assets

Place screenshot images here for the README files.

## Expected files

| File | Used in | Description |
|------|---------|-------------|
| `screenshot.png` | README.md / README_ja.md | Score editor screenshot (e.g. MusicLav) |
| `sample-score.svg` | README.md | Deterministic `acorde-render-svg` output for `tests/fixtures/simple.musicxml`. Regenerate with `cargo run -p acorde-render-svg --example render_musicxml > docs/assets/sample-score.svg` — output is deterministic, so a regenerated file with no logic changes produces an identical diff. |

## How to wire up

After adding the image, uncomment and update the line in each README:

```markdown
![Score editor — powered by acorde](docs/assets/screenshot.png)
```

For the Japanese README:
```markdown
![楽譜エディタ（acorde 使用）](docs/assets/screenshot.png)
```

## Tips

- PNG preferred; keep under 1 MB for fast page loads
- Crop to show the score canvas + toolbar; avoid OS chrome
- 1200–1600 px wide looks good on GitHub
