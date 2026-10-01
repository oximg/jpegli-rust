import subprocess,json,random
from pathlib import Path
root=Path(__file__).resolve().parents[1]
files=sorted((root/'target/comparison/corpus').glob('*.rgb'))
random.Random(782).shuffle(files)
results=[]
for f in files:
 w,h=map(int,f.stem.rsplit('-',1)[1].split('x'))
 raw=subprocess.check_output([str(root/'target/comparison/controlled/target/release/examples/investigate'),str(f),str(w),str(h),'30'])
 results.append(json.loads(raw))
 print(f.name,flush=True)
(root/'docs/investigation-raw.json').write_text(json.dumps(results,indent=2)+'\n')
