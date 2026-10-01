#!/usr/bin/env python3
"""Same-binary, old-core pipeline scan experiment; run after compilation."""
import hashlib,json,os,random,statistics,subprocess
from pathlib import Path
root=Path(__file__).resolve().parents[1]
work=root/'target/comparison'
binary=work/'oximg/target/release/examples/compare'
cases=[(p,size,parallel) for p in sorted((work/'corpus').glob('kodim*.jpg')) for size in [128,512] for parallel in [1,2]]
records=[]
original=json.loads((root/'docs/comparison-raw.json').read_text())
expected={r['case']:r['sha256'] for r in original['records'] if r['variant']=='stock'}
for round in range(5):
 random.Random(391+round).shuffle(cases)
 for index,(path,size,parallel) in enumerate(cases):
  case=f'{path.stem}:pipeline:{size}:p{parallel}'
  for scans in ([8,4] if (index+round)%2 else [4,8]):
   env={k:v for k,v in os.environ.items() if not k.startswith('OXIMG_')}
   env['JPEGLI_BENCH_SCANS']=str(scans)
   output=work/f'scan-pipeline-{scans}.jpg'
   result=json.loads(subprocess.check_output([str(binary),str(path),str(size),str(size),'pipeline','30','80',str(output),str(parallel)],env=env))
   digest=hashlib.sha256(output.read_bytes()).hexdigest()
   if scans==8: assert digest==expected[case],f'baseline changed: {case}'
   result.update(case=case,round=round,scans=scans,sha256=digest)
   records.append(result)
 print(f'round {round+1}/5',flush=True)
(root/'docs/scan-pipeline-raw.json').write_text(json.dumps({'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'records':records},indent=2)+'\n')
