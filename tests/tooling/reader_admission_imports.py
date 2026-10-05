"""Linux source-hidden trace: admission must not load the reader presentation layer.

Trace successful reader-controller opens, not failed lookup attempts. Shared
stdlib files are also read by dependency hash verification, so they are not treated
as proof of module evaluation. Timing is measured separately, without a time gate.
"""
import reader_appearance_native as harness
PREFIX=harness.PROBE.split('\ntry:\n',1)[0]
CLEANUP='\nfinally:'+harness.PROBE.rsplit('\nfinally:',1)[1]
JOURNEY=r'''
try:
    import re,signal
    expect_eager=False
    good=Path('/probe/valid.html');good.write_text('<article><h1>PRIVATE_TITLE</h1><p>Ελληνικό κείμενο.</p></article>')
    original=good.read_bytes();bad=Path('/probe/invalid.html');bad.write_bytes(b'\xff')
    targets=['examples/declarative_browser/app.sapp','examples/declarative_browser/settings_layout.sapp']
    shared=['stdlib/gui.sapp','stdlib/graphics.sapp','stdlib/modern_ui.sapp']
    observations=[]
    for name,args in [('valid',['--check','--open',str(good)]),('invalid',['--check','--open',str(bad)]),('invalid-gui',['--with-profile','--open',str(bad)]),('demo',['--check'])]:
        trace=Path('/probe/'+name+'.trace')
        headless={k:v for k,v in os.environ.items() if k not in ('DISPLAY','WAYLAND_DISPLAY')}
        p=subprocess.Popen(['strace','-f','-qq','-e','trace=openat,openat2','-o',str(trace),'/bin/sh',launch,*args],env=headless,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,start_new_session=True)
        try:out,err=p.communicate(timeout=8)
        except subprocess.TimeoutExpired:
            os.killpg(p.pid,signal.SIGKILL);p.communicate();raise
        paths=[]
        for line in trace.read_text().splitlines():
            if re.search(r'= [0-9]+$',line):paths.extend(re.findall(r'"([^"\n]*\.sapp)"',line))
        hits=[wanted for wanted in targets if any(path.endswith(wanted) for path in paths)]
        integrity_paths=[wanted for wanted in shared if any(path.endswith(wanted) for path in paths)]
        assert integrity_paths==shared,'Dependency integrity reads must remain visible'
        assert p.returncode==(0 if name in ('valid','demo') else 1) and err=='',(name,out,err)
        if name=='demo':assert hits==targets and out=='declarative_browser_pass\n'
        else:
            assert hits==(targets if expect_eager else []),(name,hits)
            assert not any(path.endswith('appearance_store.sapp') for path in paths)
            if name=='valid':assert json.loads(out)==dict(ok=True,format='pblang.reader.document.v1',bytes=len(original),blocks=1)
            elif name=='invalid':assert json.loads(out)['code']=='invalid_utf8' and json.loads(out)['ok'] is False
            else:assert out=='Document unavailable or unsupported; no document or preferences changed. The file is not valid UTF-8. Save it as UTF-8 and retry.\n'
        assert 'PRIVATE_TITLE' not in out+err and str(good) not in out+err
        observations.append(dict(case=name,exit=p.returncode,successful_sapp_opens=len(paths),unique_successful_paths=len(set(paths)),controller_modules=hits,shared_integrity_paths=integrity_paths))
    assert not profile.exists() and good.read_bytes()==original and bad.read_bytes()==b'\xff'
    print(json.dumps(dict(passed=True,baseline=expect_eager,headless=True,no_profile_or_input_writes=True,observations=observations)))
'''
if __name__=='__main__':
    harness.PROBE=PREFIX+JOURNEY+CLEANUP
    harness.main()
