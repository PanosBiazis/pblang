"""Real source-hidden launcher file import; no controller substitution or instrumentation."""
import reader_appearance_native as harness
PREFIX=harness.PROBE.split('\ntry:\n',1)[0]
PREFIX=PREFIX.replace("+(['--with-profile'] if with_profile else [])", "+(['--with-profile'] if with_profile else [])+['--open',str(document)]")
PREFIX=PREFIX.replace(";time.sleep(.4);click(700,44)",";time.sleep(.4)")
CLEANUP='\nfinally:'+harness.PROBE.rsplit('\nfinally:',1)[1]
JOURNEY=r'''
try:
    document=Path('/probe/Εισαγόμενο with spaces.html')
    source='<article><h1>Imported from launcher</h1><h2>'+('W'*256)+'</h2><p>'+('Ελληνικό κείμενο. '*12)+'</p><a href="pb://guide">Open built-in guide</a></article>'
    document.write_text(source);original=document.read_bytes()
    start(False);pid=child.pid;app=Path('/proc/'+str(pid)+'/cwd').resolve();shot('import-default')
    rejected=[]
    def reject_inputs(saved_profile=None):
        active_pid=child.pid
        for name,content,hint in [('malformed',b'<article>SECRET_SOURCE_MARKER','supported article'),('unsupported',b'<article><h1>x</h1><script>SECRET_SOURCE_MARKER</script></article>','supported article'),('oversize',b'x'*16385,'16384 bytes'),('utf8',b'\xff','Save it as UTF-8')]:
            bad=Path('/probe/'+name+'.html');bad.write_bytes(content)
            p=subprocess.run(['/bin/sh',launch,'--with-profile','--open',str(bad)],capture_output=True,text=True,timeout=5)
            assert p.returncode!=0 and hint in p.stdout and 'SECRET_SOURCE_MARKER' not in p.stdout
            assert str(bad) not in p.stdout and p.stderr==''
            assert 'Stack trace' not in p.stdout and '/app/' not in p.stdout and '/home/' not in p.stdout
            assert child.poll() is None and child.pid==active_pid and bad.read_bytes()==content
            if saved_profile is None:assert not profile.exists()
            else:assert profile.read_bytes()==saved_profile
            assert len(x('search','--name','^PBLang document browser$').stdout.splitlines())==1
            rejected.append(name)
    reject_inputs()
    shot('import-after-rejections');close()
    # Existing PBLang store writes only the synthetic harness data directory.
    writer='let Store=import('+json.dumps(str(app/'examples/declarative_browser/appearance_store.sapp'))+');let parent='+json.dumps(str(data))+';let loaded=Store.load(parent);let style=loaded.appearance;style.font_family="serif";style.body_size=40;style.title_size=40;style.font_scale=1.5;style.line_height=1.25;let saved=Store.save(parent,style,loaded.token);assert(saved.ok,"Synthetic style save failed");'
    Path('/probe/style.sapp').write_text(writer)
    p=subprocess.run([str(app/'pblang'),'run','/probe/style.sapp','--allow-all','--deny-net','--deny-http','--deny-spawn'],cwd=app,capture_output=True,text=True,timeout=6)
    assert p.returncode==0,(p.stdout,p.stderr);saved=profile.read_bytes()
    start(True);x('windowsize',window,320,480);time.sleep(.6);shot('import-large-first');key('End');shot('import-large-last');reject_inputs(saved);shot('import-large-after-rejections')
    # Last page has Home, Style, Previous page, then the imported local link.
    tabs(4);shot('import-link-focus');key('Return');shot('import-guide')
    click(44,32);shot('import-returned-home');click(275,32);shot('import-style');close()
    assert profile.read_bytes()==saved and document.read_bytes()==original
    # A damaged saved profile must not prevent reading or be silently rewritten.
    damaged=b'{invalid';profile.write_bytes(damaged)
    start(True);click(700,44);shot('import-invalid-profile');close()
    assert profile.read_bytes()==damaged and document.read_bytes()==original
    profile.write_bytes(saved)
    start(False);shot('import-session-default');close();assert profile.read_bytes()==saved
    print(json.dumps(dict(passed=True,real_launcher_import=True,invalid_launches_preserved_existing_gui=rejected,actionable_errors=True,rejections_with_saved_profile=True,invalid_profile_preserved=True,same_existing_gui_pid=True,profile_unchanged=True,input_unchanged=True,large_text=True,keyboard_navigation_exercised=True,local_link_and_back_visual_review=True,default_no_exec_no_profile_writes=True,visual_review_required=True)))
'''
if __name__=='__main__':
    harness.PROBE=PREFIX+JOURNEY+CLEANUP
    harness.main()
