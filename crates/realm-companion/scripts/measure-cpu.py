#!/usr/bin/env python3
"""Read-only macOS ps CPU-time sampling: PID OUTPUT_JSON SECONDS_PER_ROUND ROUNDS."""
import subprocess,time,json,sys,pathlib,statistics
if len(sys.argv)!=5:
 raise SystemExit("usage: measure-cpu.py PID OUTPUT_JSON SECONDS_PER_ROUND ROUNDS")
pid=int(sys.argv[1]);out=pathlib.Path(sys.argv[2]);seconds=int(sys.argv[3]);rounds=int(sys.argv[4])
if min(pid,seconds,rounds)<1:raise SystemExit("PID and measurement durations must be positive")
out.parent.mkdir(parents=True,exist_ok=True)
def reading():
 s=subprocess.check_output(['ps','-p',str(pid),'-o','time=,rss='],text=True).split();parts=s[0].split(':');cpu=sum(float(v)*60**i for i,v in enumerate(reversed(parts)));return {'t':time.monotonic(),'cpu':cpu,'rss_kib':int(s[1])}
records=[]
for run in range(rounds):
 start=previous=reading();samples=[]
 for _ in range(seconds):
  time.sleep(1);now=reading();samples.append(100*(now['cpu']-previous['cpu'])/(now['t']-previous['t']));previous=now
 result={'round':run+1,'cpu_mean':100*(now['cpu']-start['cpu'])/(now['t']-start['t']),'cpu_median':statistics.median(samples),'cpu_p95':sorted(samples)[int(.95*(len(samples)-1))],'samples':samples,'rss_kib':now['rss_kib'],'seconds':now['t']-start['t']};records.append(result);out.write_text(json.dumps({'pid':pid,'rounds':records},indent=2)+'\n');print(json.dumps({k:v for k,v in result.items() if k!='samples'}),flush=True)
