"""Rebuilds the list of scenarios in docs/TEST-SCENARIOS.md from the test sources.

    python tools/gen-test-doc.py

The "What should work" summary and the "How to run" notes at the top of the file are kept as they are (edit them
by hand); everything from "1. Unit tests" on is made again from the tests in `crates/` and `tests/ui/smoke.ps1`.
"""

import glob
import re

DOC = 'docs/TEST-SCENARIOS.md'
BS = chr(92)
NOT_TESTS = {'helper_start_a_run_and_crash'}  # started by another test, not a scenario of its own


def tests_in(path):
    lines = open(path, encoding='utf-8').read().split('\n')
    out = []
    for i, line in enumerate(lines):
        m = re.match(r'\s*(?:pub\s+)?fn (\w+)\(', line)
        if not m or m.group(1) in NOT_TESTS:
            continue
        j, attrs = i - 1, []
        while j >= 0 and (lines[j].strip().startswith('#[') or lines[j].strip().startswith('//')):
            attrs.append(lines[j].strip())
            j -= 1
        if any(a in ('#[test]', '#[gpui::test]') for a in attrs):
            out.append(m.group(1))
    return out


def pretty(name):
    s = name.replace('_', ' ').strip()
    return s[0].upper() + s[1:] + '.'


old = open(DOC, encoding='utf-8').read()
head = old[:old.index('## 1. Unit tests')]

crates = {
    'jmx-model': 'jmx-model (reading, editing and writing `.jmx` text)',
    'plan-core': 'plan-core (elements, rules, editing, forms, scope, variables, saving, scripts, plans made for Validate and Send once)',
    'shell-core': 'shell-core (settings, layout, text editing, palette, actions)',
    'jvm-runtime': 'jvm-runtime (finding Java and JMeter, running and stopping JMeter, results, live numbers, run folders)',
}
unit = []
for c, title in crates.items():
    files = sorted(glob.glob(f'crates/{c}/src/**/*.rs', recursive=True)) + sorted(glob.glob(f'crates/{c}/tests/*.rs'))
    unit.append((title, [(f.replace(BS, '/'), t) for f in files for t in tests_in(f)]))
unit.append(('app (pure logic inside the window crate)', [(f.replace(BS, '/'), t) for f in sorted(glob.glob('crates/app/src/*.rs')) for t in tests_in(f)]))

areas = [
    ('shell.rs', 'Opening plans, tree, palette, tabs, panels, themes, settings'),
    ('editing.rs', 'Editing: add, delete, move, drag and drop, undo, saving, recovery, scope, big plans'),
    ('forms.rs', 'Forms: boxes, numbers, drop-downs, tables, typing, hints, focus'),
    ('fields.rs', 'Text and number boxes'),
    ('runs.rs', 'Running plans: questions before a run, live numbers, stopping, run history, the runs folder, settings'),
]

md = []
n = 0
md.append('## 1. Unit tests\n\nThese test the logic without a window. Tests that start the real JMeter skip themselves when Java or JMeter is not installed.\n')
for title, items in unit:
    if not items:
        continue
    md.append(f'### {title}\n')
    last = None
    for f, t in items:
        if f != last:
            md.append(f'*{f}*\n')
            last = f
        n += 1
        md.append(f'{n}. {pretty(t)} — `{t}`')
    md.append('')
md.append('## 2. Whole-window tests (headless)\n\nThese run the real window without a screen, driving it with simulated keys, clicks and drags.\n')
for f, title in areas:
    ts = tests_in('crates/app/tests/' + f)
    md.append(f'### {title}\n\n*crates/app/tests/{f}*\n')
    for t in ts:
        n += 1
        md.append(f'{n}. {pretty(t)} — `{t}`')
    md.append('')
md.append('## 3. Real-window checks\n\nThese start the built app, drive it with real mouse and keyboard input, and read its state file and the files it writes (`tests/ui/smoke.ps1`).\n')
sm = open('tests/ui/smoke.ps1', encoding='utf-8').read()
sec = None
real = 0
for line in sm.split('\n'):
    m = re.match(r'Write-Host "(\d+\. [^"]+)"', line)
    if m:
        sec = m.group(1)
        md.append(f'\n### {sec}\n')
        continue
    m = re.match(r'\s*Check "([^"]+)"', line)
    if m:
        n += 1
        real += 1
        d = m.group(1)
        d = d[0].upper() + d[1:]
        md.append(f'{n}. {d}. — `smoke.ps1`, section "{sec}"')
md.append(f'\nTotal scenarios listed: {n}.')
open(DOC, 'w', encoding='utf-8', newline='\n').write(head + '\n'.join(md) + '\n')
print(n, 'scenarios;', real, 'real-window checks')
