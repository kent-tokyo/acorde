#!/usr/bin/env python3
"""Reproduce metadata-only final-v2 selection; supply the archived tree-JSON directory.
Refuses to overwrite the reserved selection. No scores are opened or fetched.
"""
import hashlib
import json
from pathlib import Path, PurePosixPath
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parents[2]
import sys

PREFIX = Path(sys.argv[1]) if len(sys.argv) > 1 else Path('/private/tmp')
SEED = 'acorde-print-final-v2-20261115'
specs = [
    ('mozart', 'DCMLab/mozart_piano_sonatas', '5337257a5318711e6302cfe85c3f1a6ade3c6271'),
    ('beethoven', 'DCMLab/beethoven_piano_sonatas', 'ea7181bff88abc8713257234f7ec4033178c57a9'),
    ('debussy-other', 'DCMLab/debussy_other_piano_pieces', '74224fd3979ddec4a4a818cca51a883f6d91ca94'),
    ('lieder', 'OpenScore/Lieder', '38c5db510224d9facdc4b08d741fc788cfb58ea8'),
    ('quartets', 'OpenScore/StringQuartets', '2614b0b2958dd1ce3f52e172a43e5f4d0e29570f'),
]

def digest(value):
    return hashlib.sha256(value).hexdigest()

def work_key(repo, path):
    parts = PurePosixPath(path).parts
    if repo == 'OpenScore/Lieder':
        return '/'.join(parts[1:3] if parts[2] != '_' else parts[1:4])
    if repo == 'OpenScore/StringQuartets':
        return '/'.join(parts[1:-1])
    return PurePosixPath(path).stem.split('-')[0]

retired_path = ROOT / 'benchmarks/print/parity-holdout-selection-v1.json'
retired = json.loads(retired_path.read_text())
excluded = {(w['repository'], work_key(w['repository'], w['path'])) for w in retired['works']}
fixtures_path = ROOT / 'tests/fixtures/manifest.json'
for fixture in json.loads(fixtures_path.read_text())['fixtures']:
    url = unquote(fixture.get('source_url', ''))
    for _, repo, _ in specs:
        marker = 'github.com/' + repo + '/blob/'
        if marker in url:
            path = url.split(marker, 1)[1].split('/', 1)[1]
            excluded.add((repo, work_key(repo, path)))

trees = {}
inventories = []
for name, repo, revision in specs:
    path = PREFIX / f'acorde-print-final-{name}-tree.json'
    tree = json.loads(path.read_text())
    assert tree['sha'] == revision and not tree['truncated']
    trees[repo] = tree['tree']
    inventories.append({'repository': repo, 'revision': revision, 'metadata_file': str(path),
                        'metadata_sha256': digest(path.read_bytes())})

selected = []
selected_work_keys = set(excluded)

def choose(group, repo, count, unique_composers=False):
    revision = next(revision for _, r, revision in specs if r == repo)
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
    for rank, entry, identity in sorted(candidates):
        path = entry['path']
        if identity in selected_work_keys:
            continue
        composer = PurePosixPath(path).parts[1]
        if unique_composers and composer in composers:
            continue
        row = {'group': group, 'id': PurePosixPath(path).stem, 'repository': repo,
               'revision': revision, 'path': path, 'git_blob_sha1': entry['sha'],
               'bytes': entry['size'], 'work_id': identity[1], 'selection_rank': rank,
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

for _, repo, _ in specs[:3]:
    choose('piano', repo, 2)
choose('vocal', 'OpenScore/Lieder', 6, True)
choose('chamber', 'OpenScore/StringQuartets', 6, True)
choose('part', 'OpenScore/StringQuartets', 6, True)
assert len(selected) == 24
result = {
    'schema_version': 1,
    'classification': 'reserved final v2 metadata selection; score content and renderings unopened',
    'selected_on': '2026-11-15',
    'seed': SEED,
    'selection_rule': 'Ascending SHA-256(seed, group, repository, path), complete files with no size, difficulty or feature filters. Two different piano works per composer; six different composers each in vocal, chamber and part groups. Exclude all retired works/cycles and matching imported repository fixtures. Related movements/cycle members remain in one cohort. Parts use the first eight rank hex digits modulo four. This is deterministic coverage sampling, not a random population sample.',
    'stop_rule': 'Freeze candidate, font, source and shared-input hashes before any final render. Any failed or inspected-to-tune cohort becomes development; all failures remain recorded and a later final cohort must be disjoint.',
    'retired_selection_sha256': digest(retired_path.read_bytes()),
    'fixture_manifest_sha256_at_selection': digest(fixtures_path.read_bytes()),
    'metadata_inventories': inventories,
    'excluded_work_keys': [list(x) for x in sorted(excluded)],
    'works': selected,
}
out = ROOT / 'benchmarks/print/parity-holdout-selection-v2.json'
assert not out.exists()
out.write_text(json.dumps(result, indent=2, ensure_ascii=False) + '\n')
print('selected', len(selected))
for row in selected:
    print(row['group'], row['repository'], row['path'], row.get('part_index', ''))
