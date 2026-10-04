"""Test-only filesystem adversarial checks for the PBLang reader profile store."""
import argparse,base64,fcntl,hashlib,json,os,subprocess,tempfile
from pathlib import Path

def main():
 p=argparse.ArgumentParser();p.add_argument('--runtime',type=Path,required=True);p.add_argument('--evidence',type=Path,required=True);a=p.parse_args()
 repo=Path(__file__).resolve().parents[2];runtime=a.runtime.resolve();evidence=a.evidence.absolute();evidence.mkdir(mode=0o700)
 results=[]
 def check(name,ok):
  results.append(dict(name=name,passed=bool(ok)));(evidence/'acceptance.json').write_text(json.dumps(results,indent=2));print(name,bool(ok),flush=True);assert ok,name
 def call(*args):
  proc=subprocess.run([str(runtime),'run','tests/fixtures/reader_appearance_store_probe.sapp','--allow-all','--deny-spawn','--deny-net','--deny-http','--',*map(str,args)],cwd=repo,capture_output=True,text=True,timeout=30)
  assert proc.returncode==0,proc.stderr
  value=json.loads(proc.stdout)
  with (evidence/'calls.jsonl').open('a') as f:f.write(json.dumps(dict(action=args[0],result=value))+'\n')
  return value
 with tempfile.TemporaryDirectory(prefix='reader-style-test-') as temporary:
  parent=Path(temporary);data=parent/'data';data.mkdir(mode=0o700);folder=data/'reader-style';current=folder/'appearance-v1.json'
  denied=subprocess.run([str(runtime),'run','tests/fixtures/reader_appearance_store_probe.sapp','--allow-all','--deny-exec','--deny-spawn','--deny-net','--deny-http','--','load',str(data)],cwd=repo,capture_output=True,text=True,timeout=30)
  check('exec denial gives read only session fallback',denied.returncode==0 and not json.loads(denied.stdout)['can_save'] and not folder.exists())
  initial=call('load',data);default=initial['appearance'];style=dict(default,font_family='serif',body_size=20,font_scale=1.5,line_height=1.75)
  check('missing load is read only',initial['ok'] and initial['can_save'] and not folder.exists())
  saved=call('save',data,json.dumps(style),initial['token']);check('explicit save succeeds with spawn and network denied',saved['ok'] and saved['confirmed'] and saved['owned_handles_closed'])
  first=current.read_bytes();loaded=call('load',data);check('restart loads saved appearance',loaded['ok'] and loaded['appearance']==style)
  check('private file and directory',current.stat().st_mode&0o777==0o600 and folder.stat().st_mode&0o777==0o700)
  check('independent revision token',loaded['token']==hashlib.sha256(base64.b64encode(first)).hexdigest())
  same=call('save',data,json.dumps(style),loaded['token']);check('same save is no op',same['ok'] and not same['attempted'] and not list(folder.glob('previous-*')))
  stale=call('save',data,json.dumps(default),'missing');check('stale window refuses overwrite',not stale['ok'] and stale['reload'] and current.read_bytes()==first)
  with (folder/'style.lock').open('r+') as lock:
   fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB);busy=call('save',data,json.dumps(default),loaded['token']);check('concurrent save refused',not busy['ok'] and not busy['attempted'] and current.read_bytes()==first)
  reset=call('save',data,json.dumps(default),loaded['token']);check('reset defaults publishes',reset['ok'] and call('load',data)['appearance']==default)
  check('previous profile retained', (folder/f"previous-{loaded['token']}.json").read_bytes()==first)
  current.write_bytes(b'{broken');bad=call('load',data);check('invalid profile defaults without rewriting',not bad['ok'] and bad['can_save'] and bad['appearance']==default and current.read_bytes()==b'{broken')
  repaired=call('save',data,json.dumps(style),bad['token']);check('explicit repair preserves invalid bytes',repaired['ok'] and (folder/f"previous-{bad['token']}.json").read_bytes()==b'{broken')
  current.write_bytes(b'\xff');invalid=call('load',data);check('invalid UTF8 defaults',not invalid['ok'] and invalid['appearance']==default)
  utf8=call('save',data,json.dumps(default),invalid['token']);check('invalid UTF8 backup preserved',utf8['ok'] and (folder/f"previous-{invalid['token']}.json").read_bytes()==b'\xff')
  stable=current.read_bytes();current.write_bytes(b'x'*4097);large=call('load',data)
  check('oversized profile unavailable unchanged',not large['can_save'] and current.read_bytes()==b'x'*4097)
  refusal=call('save',data,json.dumps(style),utf8['token']);check('oversized profile never replaced',not refusal['ok'] and not refusal['attempted'] and current.stat().st_size==4097)
  current.write_bytes(stable);outside=parent/'outside';outside.write_bytes(stable);outside.chmod(0o600);current.unlink();current.symlink_to(outside)
  linked=call('load',data);write=call('save',data,json.dumps(style),'missing');check('symlink target preserved',not linked['can_save'] and not write['ok'] and current.is_symlink() and outside.read_bytes()==stable)
  current.unlink();os.link(outside,current);linked=call('load',data);check('hardlinked profile refused',not linked['can_save'] and outside.read_bytes()==stable)
  current.unlink();current.write_bytes(stable);current.chmod(0o644);check('non private profile refused',not call('load',data)['can_save']);current.chmod(0o600)
  foreign=parent/'foreign';foreign.mkdir(mode=0o700);(foreign/'reader-style').mkdir(mode=0o700);(foreign/'reader-style/sentinel').write_text('keep')
  check('foreign folder refused',not call('load',foreign)['can_save'] and not call('save',foreign,json.dumps(style),'missing')['ok'] and sorted(x.name for x in (foreign/'reader-style').iterdir())==['sentinel'])
  for i in range(16):(folder/f'previous-fixture-{i}.json').write_text('retained')
  before=current.read_bytes();limit=call('save',data,json.dumps(style),call('load',data)['token']);check('backup limit preserves current',not limit['ok'] and not limit['attempted'] and current.read_bytes()==before)
  check('application directory cannot be data',call('parent',repo,repo/'inside') is None)
  check('explicit private parent is selected',call('parent',repo,data)==str(data))
if __name__=='__main__':main()
