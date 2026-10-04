"""Source-hidden real launcher admission, including hostile filenames and bounded files."""
import reader_appearance_native as harness
PREFIX=harness.PROBE.split('\ntry:\n',1)[0]
CLEANUP='\nfinally:'+harness.PROBE.rsplit('\nfinally:',1)[1]
JOURNEY=r'''
try:
    checks=[];root=Path('/probe/inputs');root.mkdir();source='<article><h1>PRIVATE_TITLE</h1><p>Ελληνικό κείμενο.</p><a href="pb://guide">Guide</a></article>'
    good=root/'Έγγραφο with spaces.html';good.write_text(source)
    def run(args,valid=False):
        headless={k:v for k,v in os.environ.items() if k not in ('DISPLAY','WAYLAND_DISPLAY')}
        p=subprocess.run(['/bin/sh',launch,*args],cwd=root,env=headless,capture_output=True,text=True,timeout=5)
        assert ('PRIVATE_TITLE' not in p.stdout+p.stderr and 'SECRET_SOURCE_MARKER' not in p.stdout+p.stderr)
        if valid:
            result=json.loads(p.stdout);assert p.returncode==0 and result['ok'] and result['format']=='pblang.reader.document.v1',(args,p.stdout,p.stderr)
        else:assert p.returncode!=0,(args,p.stdout,p.stderr)
        return p
    def test(name,path,valid=False,code='read_failed'):
        result=run(['--check','--open',str(path)],valid)
        assert str(root) not in result.stdout+result.stderr
        if valid:assert set(json.loads(result.stdout))=={'ok','format','bytes','blocks'}
        elif code is not None:
            diagnostic=json.loads(result.stdout)
            assert set(diagnostic)=={'ok','format','code','error'} and diagnostic['ok'] is False
            assert diagnostic['format']=='pblang.reader.document.v1' and diagnostic['code']==code,(name,diagnostic)
            assert result.stderr=='' and 'no document or preferences changed' in diagnostic['error']
            assert 'Stack trace' not in result.stdout and '/app/' not in result.stdout and '/home/' not in result.stdout
        checks.append(dict(name=name,passed=True))
    test('relative Unicode and spaces',good.name,True)
    test('absolute Unicode and spaces',good,True)
    for name in ('root','literal $(touch unwanted) ; quoted " name.html'):
        p=root/name;p.write_text(source);test('literal filename '+name,p,True);assert p.read_text()==source
    nested=root/'nested';nested.mkdir();test('lexical relative dot segments','nested/../'+good.name,True)
    body=source.encode();exact=root/'exact.html';exact.write_bytes(body+b' '*(16384-len(body)));test('exact byte limit with outer whitespace',exact,True)
    invalid={
        'whitespace':b' \n\t', 'oversize':body+b' '*(16385-len(body)), 'empty':b'', 'utf8':b'\xff',
        'malformed':b'<article><h1>SECRET_SOURCE_MARKER</h1>',
        'script':b'<article><h1>x</h1><script>SECRET_SOURCE_MARKER</script></article>',
        'remote':b'<article><h1>x</h1><a href="https://example.com/">link</a></article>',
        'nested':b'<article><h1>x</h1><p><b>x</b></p></article>',
        'attributes':b'<article><h1>x</h1><p onclick="run">x</p></article>',
        'title':('<article><h1>'+'x'*81+'</h1></article>').encode(),
        'block':('<article><h1>x</h1><p>'+'x'*257+'</p></article>').encode(),
        'blocks':('<article><h1>x</h1>'+'<p>x</p>'*9+'</article>').encode(),
        'link':('<article><h1>x</h1><a href="pb://home">'+'x'*65+'</a></article>').encode(),
    }
    for name,content in invalid.items():
        p=root/(name+'.html');p.write_bytes(content);test(name,p,code={'oversize':'too_large','empty':'empty_document','whitespace':'empty_document','utf8':'invalid_utf8'}.get(name,'unsupported_document'));assert p.read_bytes()==content
    maximum=root/'maximum.html';maximum.write_text('<article><h1>'+'x'*80+'</h1>'+'<p>'+('x'*256)+'</p>'+('<p>x</p>'*6)+'<a href="pb://home">'+('x'*64)+'</a></article>');test('maximum admitted fields',maximum,True)
    sparse=root/'sparse.html'
    with sparse.open('wb') as f:f.truncate(100*1024*1024)
    test('100 MiB sparse input rejected',sparse,code='too_large');assert sparse.stat().st_size==100*1024*1024
    leaf=root/'symlink.html';leaf.symlink_to(good);test('leaf symlink refused',leaf)
    linked=root/'linked-parent';linked.symlink_to(root,target_is_directory=True);test('parent symlink refused',linked/good.name)
    fifo=root/'fifo';os.mkfifo(fifo);test('FIFO refuses without blocking',fifo)
    test('directory refused',nested);test('missing file refused',root/'missing.html')
    hard=root/'hardlink.html';os.link(good,hard);test('read-only hardlink accepted',hard,True);assert hard.read_bytes()==body
    control=root/'newline\nname.html';control.write_text(source);test('newline filename refused',control,code=None)
    control=root/'tab\tname.html';control.write_text(source);test('tab filename diagnosed',control,code='invalid_path')
    for args in [[],['--open'],['--open',''],['--open',str(good),'--open',str(good)],['--check','--with-profile'],['--help','--open',str(good)],['--unknown']]:
        if not args:continue
        run(args);checks.append(dict(name='invalid arguments '+repr(args),passed=True))
    run(['--open',str(good),'--check'],True);checks.append(dict(name='option order preserved',passed=True))
    assert not profile.exists() and not (root/'unwanted').exists();assert good.read_bytes()==body
    print(json.dumps(dict(passed=True,checks=checks,check_count=len(checks),headless_check=True,no_source_disclosure=True,no_input_or_profile_writes=True)))
'''
if __name__=='__main__':
    harness.PROBE=PREFIX+JOURNEY+CLEANUP
    harness.main()
