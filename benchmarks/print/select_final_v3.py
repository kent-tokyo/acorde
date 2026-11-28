#!/usr/bin/env python3
"""Reproduce the metadata-only final-v3 print holdout selection (#499).

Usage: select_final_v3.py CLONES_DIR

CLONES_DIR holds blob-less, checkout-free clones made with
`git clone --depth 1 --filter=blob:none --no-checkout URL OWNER_NAME`, one per
repository below (for example `OpenScore_Lieder`). Each clone must be at the
pinned revision. The script lists trees with `git ls-tree`, writes the tree
inventories it used next to the clones, and reads only the byte size of each
selected blob. No score is parsed or rendered. It refuses to overwrite the
reserved selection.
"""
import hashlib
import json
import subprocess
import sys
from pathlib import Path, PurePosixPath
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parents[2]
CLONES = Path(sys.argv[1])
SEED = 'acorde-print-final-v3-20261128'
# Piano repositories not used by v1 or v2, so no piano work can repeat.
PIANO = [
    ('DCMLab/grieg_lyric_pieces', '91a304563521f3f273b8c0aadec1ce2ede2d1384'),
    ('DCMLab/tchaikovsky_seasons', '5af15033c5f9c282f38fcf71234b86349e61e8c3'),
    ('DCMLab/dvorak_silhouettes', 'f228006fcd8696c809cfc8e701ed215cec3d07f1'),
]
LIEDER = ('OpenScore/Lieder', '38c5db510224d9facdc4b08d741fc788cfb58ea8')
QUARTETS = ('OpenScore/StringQuartets', '2614b0b2958dd1ce3f52e172a43e5f4d0e29570f')
RETIRED = ['parity-holdout-selection-v1.json', 'parity-holdout-selection-v2.json']


def digest(value):
    return hashlib.sha256(value).hexdigest()


def git(repo, *args):
    clone = CLONES / repo.replace('/', '_')
    return subprocess.run(['git', '-c', 'gc.auto=0', '-C', str(clone), *args],
                          check=True, capture_output=True, text=True).stdout


def work_key(repo, path):
    parts = PurePosixPath(path).parts
    if repo == 'OpenScore/Lieder':
        return '/'.join(parts[1:3] if parts[2] != '_' else parts[1:4])
    if repo == 'OpenScore/StringQuartets':
        return '/'.join(parts[1:-1])
    return PurePosixPath(path).stem.split('-')[0]


excluded = set()
retired_hashes = {}
for name in RETIRED:
    path = ROOT / 'benchmarks/print' / name
    retired_hashes[name] = digest(path.read_bytes())
    for work in json.loads(path.read_text())['works']:
        excluded.add((work['repository'], work_key(work['repository'], work['path'])))
fixtures_path = ROOT / 'tests/fixtures/manifest.json'
for fixture in json.loads(fixtures_path.read_text())['fixtures']:
    url = unquote(fixture.get('source_url', ''))
    for repo, _ in PIANO + [LIEDER, QUARTETS]:
        marker = 'github.com/' + repo + '/blob/'
        if marker in url:
            path = url.split(marker, 1)[1].split('/', 1)[1]
            excluded.add((repo, work_key(repo, path)))

trees = {}
inventories = []
for repo, revision in PIANO + [LIEDER, QUARTETS]:
    if git(repo, 'rev-parse', 'HEAD').strip() != revision:
        raise ValueError(f'{repo} clone is not at {revision}')
    entries = []
    for line in git(repo, 'ls-tree', '-r', 'HEAD').splitlines():
        meta, path = line.split('\t', 1)
        _, kind, sha = meta.split()
        entries.append({'path': path, 'type': kind, 'sha': sha})
    inventory = CLONES / f'acorde-print-final-v3-{repo.replace("/", "_")}-tree.json'
    inventory.write_text(json.dumps({'sha': revision, 'truncated': False, 'tree': entries},
                                    indent=1) + '\n')
    trees[repo] = entries
    inventories.append({'repository': repo, 'revision': revision,
                        'metadata_file': inventory.name,
                        'metadata_sha256': digest(inventory.read_bytes())})

selected = []
selected_work_keys = set(excluded)


def choose(group, repo, revision, count, unique_composers=False):
    candidates = []
    for entry in trees[repo]:
        path = entry['path']
        p = PurePosixPath(path)
        if entry['type'] != 'blob':
            continue
        if group == 'piano':
            if p.suffix != '.mscx' or p.parent.as_posix() != 'MS3':
                continue
        elif not (path.startswith('scores/') and p.suffix == '.mxl'):
            continue
        identity = (repo, work_key(repo, path))
        if identity in selected_work_keys:
            continue
        rank = digest(f'{SEED}\n{group}\n{repo}\n{path}'.encode())
        candidates.append((rank, entry, identity))
    composers = set()
    added = 0
    for rank, entry, identity in sorted(candidates, key=lambda c: c[0]):
        path = entry['path']
        if identity in selected_work_keys:
            continue
        composer = PurePosixPath(path).parts[1]
        if unique_composers and composer in composers:
            continue
        row = {'group': group, 'id': PurePosixPath(path).stem, 'repository': repo,
               'revision': revision, 'path': path, 'git_blob_sha1': entry['sha'],
               'bytes': int(git(repo, 'cat-file', '-s', entry['sha'])),
               'work_id': identity[1], 'selection_rank': rank,
               'license': 'CC0-1.0' if repo.startswith('OpenScore/') else 'CC-BY-NC-SA-4.0'}
        if group == 'part':
            row['part_index'] = int(rank[:8], 16) % 4
        selected.append(row)
        selected_work_keys.add(identity)
        composers.add(composer)
        added += 1
        if added == count:
            return
    raise ValueError(f'not enough metadata candidates for {group}/{repo}')


for repo, revision in PIANO:
    choose('piano', repo, revision, 2)
choose('vocal', *LIEDER, 6, True)
choose('chamber', *QUARTETS, 6, True)
choose('part', *QUARTETS, 6, True)
assert len(selected) == 24
result = {
    'schema_version': 1,
    'classification': 'reserved final v3 metadata selection; score content and renderings unopened',
    'selected_on': '2026-11-28',
    'seed': SEED,
    'selection_rule': 'Ascending SHA-256(seed, group, repository, path), complete files with no size, difficulty or feature filters. Two different piano works from each of three DCMLab corpora unused by v1 and v2; six different composers each in vocal, chamber and part groups. Exclude every v1 and v2 work/cycle and matching imported repository fixtures. Related movements/cycle members remain in one cohort. Parts use the first eight rank hex digits modulo four. This is deterministic coverage sampling, not a random population sample.',
    'stop_rule': 'Freeze candidate, font, source and shared-input hashes before any final render. Run once, after the development corpus issue (#498) is closed. Do not inspect outputs while changing algorithms. Any failed or inspected-to-tune cohort becomes development; all failures remain recorded and a later final cohort must be disjoint.',
    'retired_selection_sha256': retired_hashes,
    'fixture_manifest_sha256_at_selection': digest(fixtures_path.read_bytes()),
    'metadata_inventories': inventories,
    'excluded_work_keys': [list(x) for x in sorted(excluded)],
    'works': selected,
}
out = ROOT / 'benchmarks/print/parity-holdout-selection-v3.json'
assert not out.exists()
out.write_text(json.dumps(result, indent=2, ensure_ascii=False) + '\n')
print('selected', len(selected))
for row in selected:
    print(row['group'], row['repository'], row['path'], row.get('part_index', ''))
