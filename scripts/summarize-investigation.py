#!/usr/bin/env python3
import json, math, statistics
from collections import defaultdict
from pathlib import Path
root=Path(__file__).resolve().parents[1]
gm=lambda xs: math.exp(statistics.mean(map(math.log,xs)))
def grouped(name):
    groups=defaultdict(list)
    for d in json.loads((root/'docs'/name).read_text()):
        groups['4K synthetic' if d['width']>1000 else str(max(d['width'],d['height']))].append(d)
    return groups
lines=['| Edge | New | Write | Finish | Copy | Sequential time / 8 | Sequential size / 8 | Default time / 8 |','|---|---:|---:|---:|---:|---:|---:|---:|']
for group,ds in sorted(grouped('investigation-raw.json').items()):
    phases=defaultdict(list); ratios=defaultdict(list)
    for d in ds:
        vs={v:[r for r in d['records'] if r['variant']==v] for v in ['native','vec','sequential','default']}
        med=lambda v,k:statistics.median(r[k] for r in vs[v])
        for k in ['new_ms','write_ms','finish_ms','copy_ms']: phases[k].append(med('vec',k)/med('vec','total_ms'))
        for v in ['sequential','default']:
            ratios[v].append(med(v,'total_ms')/med('native','total_ms'))
            ratios[v+'size'].append(vs[v][0]['bytes']/vs['native'][0]['bytes'])
    lines.append('| '+group+' | '+' | '.join(f'{statistics.mean(phases[k])*100:.2f}%' for k in ['new_ms','write_ms','finish_ms','copy_ms'])+' | '+f"{gm(ratios['sequential']):.3f}x | {gm(ratios['sequentialsize']):.3f}x | {gm(ratios['default']):.3f}x |")
for name in ['scan-search-stock-raw.json','scan-search-raw.json']:
    lines+=['',name,'','| Edge | Scans | Time / 8 | Size / 8 |','|---|---:|---:|---:|']
    for group,ds in sorted(grouped(name).items()):
        for variant in ['4','5','6']:
            ts=[];sizes=[]
            for d in ds:
                a=[r for r in d['records'] if r['variant']==variant];b=[r for r in d['records'] if r['variant']=='8']
                ts.append(statistics.median(r['total_ms'] for r in a)/statistics.median(r['total_ms'] for r in b));sizes.append(a[0]['bytes']/b[0]['bytes'])
            lines.append(f'| {group} | {variant} | {gm(ts):.4f}x | {gm(sizes):.4f}x |')
rs=json.loads((root/'docs/scan-pipeline-raw.json').read_text())['records'];pairs=defaultdict(dict)
for r in rs:pairs[(r['case'],r['round'])][r['scans']]=r
lines+=['','Old-core pipeline, four scans vs eight:','','| Edge | Parallel | Time ratio | Round geomean range | Size ratio |','|---|---:|---:|---:|---:|']
for size in [128,512]:
    for p in [1,2]:
        selected=[(r,v) for (c,r),v in pairs.items() if c.endswith(f':{size}:p{p}')]
        ratio=lambda v:statistics.median(v[4]['samples_ms'])/statistics.median(v[8]['samples_ms'])
        rt=[ratio(v) for r,v in selected];sz=[v[4]['bytes']/v[8]['bytes'] for r,v in selected]
        rounds=[gm([ratio(v) for r,v in selected if r==n]) for n in range(5)]
        lines.append(f'| {size} | {p} | {gm(rt):.4f}x | {min(rounds):.4f}–{max(rounds):.4f}x | {gm(sz):.4f}x |')
(root/'docs/investigation-summary.txt').write_text('\n'.join(lines)+'\n')
print('\n'.join(lines))
