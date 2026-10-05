"""Create regular-file proc fixtures; these are not live kernel processes."""
import argparse
from pathlib import Path
parser=argparse.ArgumentParser()
parser.add_argument('root',type=Path)
parser.add_argument('--count',type=int,required=True)
args=parser.parse_args()
assert 1<=args.count<=100000
args.root.mkdir(parents=True,exist_ok=False)
for pid in range(1,args.count+1):
    directory=args.root/str(pid);directory.mkdir()
    fields=['0']*22
    fields[0]='S';fields[11]=str(pid%500);fields[19]=str(pid*100);fields[21]=str(200+pid%1000)
    (directory/'stat').write_text(f'{pid} (worker-{pid}) '+' '.join(fields))
print(f'{args.count} process fixtures: {args.root}')
