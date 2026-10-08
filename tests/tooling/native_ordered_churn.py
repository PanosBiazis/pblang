"""Exercise checked binding while unrelated test windows disappear on Xvfb."""
import argparse,ctypes,json,os,shlex,signal,subprocess,tempfile,time
from pathlib import Path
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--runtime',type=Path,required=True);p.add_argument('--evidence',type=Path,required=True);a=p.parse_args()
 display=os.environ.get('DISPLAY','').split('.')[0];servers=subprocess.check_output(['ps','-C','Xvfb','-o','args='],text=True)
 assert display and any(display in shlex.split(line)[1:] for line in servers.splitlines()),'isolated Xvfb required'
 a.evidence.mkdir(mode=0o700);repo=Path(__file__).resolve().parents[2]
 x=ctypes.CDLL('/usr/lib/x86_64-linux-gnu/libX11.so.6');ptr=ctypes.c_void_p;ulong=ctypes.c_ulong;integer=ctypes.c_int
 for name,args,result in [('XOpenDisplay',[ctypes.c_char_p],ptr),('XDefaultRootWindow',[ptr],ulong),('XCreateSimpleWindow',[ptr,ulong,integer,integer,ctypes.c_uint,ctypes.c_uint,ctypes.c_uint,ulong,ulong],ulong),('XDestroyWindow',[ptr,ulong],integer),('XSync',[ptr,integer],integer),('XCloseDisplay',[ptr],integer)]:
  fn=getattr(x,name);fn.argtypes=args;fn.restype=result
 d=x.XOpenDisplay(None);assert d;root=x.XDefaultRootWindow(d);child=None
 try:
  with tempfile.TemporaryDirectory(prefix='pb-churn-input-') as tmp:
   saved=Path(tmp)/'typed.txt';env=dict(os.environ,LANG='C.UTF-8',XMODIFIERS='@im=none',PBLANG_INPUT_DIAG_OUT=str(saved))
   child=subprocess.Popen([str(a.runtime.resolve()),'run','tests/fixtures/native_ordered_churn.sapp','--allow-all','--deny-exec','--deny-net','--deny-http','--deny-spawn'],cwd=repo,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,start_new_session=True)
   assert child.stdout.readline().strip()=='observer-ready'
   for _ in range(50):
    window=x.XCreateSimpleWindow(d,root,0,0,16,16,0,0,0);assert window;x.XDestroyWindow(d,window)
   x.XSync(d,0)
   assert child.stdout.readline().strip()=='owned-window-ready'
   window=subprocess.check_output(['xdotool','search','--name','^PBLang Churn Acceptance$'],text=True,timeout=3).splitlines()[0]
   subprocess.run(['xdotool','windowfocus','--sync',window],check=True,timeout=3)
   subprocess.run(['xdotool','type','--delay','1','abcdef'],check=True,timeout=3)
   out,err=child.communicate(timeout=5);assert child.returncode==0,(out,err)
   result=json.loads(out.strip().splitlines()[-1]);assert result['value']==saved.read_text()=='abcdef',result
   result.update(passed=True,unrelated_windows_created_and_destroyed=50,exact_save_readback=True)
   (a.evidence/'summary.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
 finally:
  if child is not None and child.poll() is None:
   os.killpg(child.pid,signal.SIGTERM)
   try:child.communicate(timeout=2)
   except subprocess.TimeoutExpired:os.killpg(child.pid,signal.SIGKILL);child.communicate()
  x.XCloseDisplay(d)
if __name__=='__main__':main()
