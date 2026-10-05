"""Source-hidden semantic checks and host-diagnostic parity for VM host errors."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import tomllib


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--candidate', type=Path, required=True)
    ap.add_argument('--report', type=Path, required=True)
    ap.add_argument('--suite', type=Path)
    ap.add_argument('--reference', type=Path)
    ap.add_argument('--calls', action='store_true')
    ap.add_argument('--numeric', action='store_true')
    ap.add_argument('--strings', action='store_true')
    ap.add_argument('--exact-math', action='store_true')
    ap.add_argument('--constructors', action='store_true')
    ap.add_argument('--start', type=int, default=0)
    ap.add_argument('--count', type=int, default=3)
    a = ap.parse_args()
    release = a.candidate.resolve()
    rows = []
    with tempfile.TemporaryDirectory(prefix='pb-arithmetic-') as name:
        root = Path(name)
        denies = ['--deny-' + p for p in ['net', 'http', 'exec', 'ffi', 'spawn', 'gfx', 'sqlite']]

        def run(source, selected=release):
            # Older semantic suites use project-relative stdlib imports. Resolve
            # them only to the selected immutable release, never the checkout.
            (root / 'stdlib').unlink(missing_ok=True)
            (root / 'stdlib').symlink_to(selected / 'stdlib', target_is_directory=True)
            manifest = ('[package]\nname="arithmetic-fixture"\nversion="1.0.0"\nentry="main.sapp"\n'
                        '[dependencies]\nstdlib={path=' + json.dumps(str(selected / 'stdlib')) + '}\n')
            (root / 'pblang.toml').write_text(manifest)
            dep = tomllib.loads((selected / 'pblang.lock').read_text())['dependencies']['stdlib']
            (root / 'pblang.lock').write_text(
                'manifest_sha256 = "' + hashlib.sha256(manifest.encode()).hexdigest() + '"\n'
                'content_hash_algorithm = "pblang-package-tree-v2"\n[dependencies.stdlib]\n'
                'path = ' + json.dumps(str(selected / 'stdlib')) + '\nsha256 = ' + json.dumps(dep['sha256']) + '\n')
            box = ['bwrap', '--unshare-all', '--die-with-parent']
            for path in ['/usr', '/bin', '/lib', '/lib64']:
                box += ['--ro-bind', path, path]
            box += ['--proc', '/proc', '--dev', '/dev', '--tmpfs', '/tmp',
                    '--ro-bind', str(selected), str(selected), '--bind', name, name, '--chdir', name]
            (root / 'main.sapp').write_text(source)
            p = subprocess.run(box + [str(selected / 'bin/pblang'), 'run', str(root / 'main.sapp'), *denies],
                               capture_output=True, text=True, timeout=15,
                               env=os.environ | {'PBLANG_CHECK_PROFILE': ''})
            errors = re.findall(r'^error\[.*?\]: (.*)$', p.stderr, re.M)
            return dict(exit=p.returncode, stdout=p.stdout, error=errors[0] if errors else None)

        def record(label, passed, **details):
            rows.append(dict(label=label, passed=passed, **details))
            a.report.write_text(json.dumps(rows, indent=2, ensure_ascii=False))
            print(label, passed, flush=True)
            assert passed, rows[-1]

        if a.suite:
            result = run(a.suite.read_text())
            record(a.suite.name, result['exit'] == 0 and '_pass' in result['stdout'], result=result)
            return

        cases = [(op, 'fn f(a,b){return a' + op + 'b;}f(nil,2);', False)
                 for op in ['+', '-', '*', '/', '%', '>', '>=', '<', '<=']]
        cases += [
            ('exact division error', 'fn f(a,b){return a/b;}f(bigint("1"),bigint("0"));', False),
            ('valid', 'fn f(a,b){print a+b;print a-b;print a*b;print a/b;print a%b;'
             'print a>b;print a>=b;print a<b;print a<=b;}f(7,2);'
             'print "α"+"β";print bigint("4")+bigint("2");'
             'print rational(3,1)/rational(2,1);', True),
            ('handled', 'fn f(a,b){return a-b;}memory_alloc(7);print try f(nil,2) catch memory_get(1);', True),
        ]
        prefix = ('let F=import("stdlib:compiler/selfhost_frontend.sapp");'
                  'let V=import("stdlib:compiler/vm.sapp");'
                  'let O=import("stdlib:compiler/optimizer.sapp");'
                  'let P=import("stdlib:selfhost_pbck.sapp");')
        if a.calls:
            assert a.reference, '--reference is required for existing VM diagnostics'
            cases = [('call-number', 'let f=3;f();', False),
                     ('call-nil', 'let f=nil;f();', False),
                     ('call-map', 'let f=map{};f();', False),
                     ('call-parameter', 'fn call(f){return f();}call(3);', False),
                     ('call-large', 'let f=3;f(1,2,3,4,5);', False),
                     ('call-caught', 'let f=3;print try f() catch 7;', True)]
        if a.numeric:
            assert a.reference, '--reference is required for existing VM diagnostics'
            assert not a.calls, 'select either numeric or non-callable cases'
            unary = ['num_floor', 'num_ceil', 'num_abs', 'num_sqrt', 'num_ln', 'num_exp']
            cases = [(name, 'fn call(f,x){return f(x);}call(' + name + ',nil);', False)
                     for name in unary]
            cases += [(name + '/' + values, 'fn call(f,a,b){return f(a,b);}call(' + name + ',' + values + ');', False)
                      for name in ['num_min', 'num_max'] for values in ['nil,2', '2,nil']]
            cases += [('generic/' + name, 'let obj=map{__add__:' + name + '};obj+2;', False)
                      for name in ['num_min', 'num_max']]
            cases += [('valid-numeric', 'print num_floor(3.9);print num_ceil(-3.9);print num_abs(-4);'
                       'print num_sqrt(9);print num_ln(1);print num_exp(0);print num_min(3,4);print num_max(3,4);', True),
                      ('caught-numeric', 'fn call(f,x){return f(x);}print try call(num_sqrt,nil) catch memory_get(1);', True),
                      ('numeric-arity', 'fn call(f){return f();}call(num_floor);', False),
                      ('numeric-exact-rejected', 'fn call(f,x){return f(x);}call(num_abs,bigint("7"));', False)]
            cases += [(name + '/both-invalid', 'fn call(f,a,b){return f(a,b);}call(' + name + ',nil,"bad");', False)
                      for name in ['num_min', 'num_max']]
        if a.strings:
            assert a.reference, '--reference is required for existing VM diagnostics'
            assert not (a.calls or a.numeric), 'select one diagnostic group'
            cases = [(name, 'fn call(f,x){return f(x);}call(' + name + ',nil);', False)
                     for name in ['str_upper', 'str_lower', 'str_trim']]
            cases += [(name + '/' + values, 'fn call(f,a,b){return f(a,b);}call(' + name + ',' + values + ');', False)
                      for name in ['str_split', 'str_contains', 'str_starts_with', 'str_ends_with', 'str_index_of']
                      for values in ['nil,"x"', '"x",nil']]
            cases += [('generic/' + name, 'let obj=map{__add__:' + name + '};obj+"x";', False)
                      for name in ['str_contains', 'str_split']]
            cases += [('valid-strings', 'print str_upper("Straße");print str_lower("ΑΒ");print str_trim(" α😀β ");'
                       'print str_split("α😀","");print str_contains("α😀β","😀");print str_starts_with("α😀β","α");'
                       'print str_ends_with("α😀β","β");print str_index_of("α😀β","β");print str_index_of("abc","z");', True),
                      ('caught-string', 'fn call(f,x){return f(x);}print try call(str_upper,nil) catch memory_get(1);', True),
                      ('string-arity', 'fn call(f){return f();}call(str_upper);', False),
                      ('string-exact-rejected', 'fn call(f,x){return f(x);}call(str_upper,bigint("7"));', False),
                      ('string-both-invalid', 'fn call(f,a,b){return f(a,b);}call(str_contains,nil,7);', False)]
        if a.exact_math:
            assert a.reference and not (a.calls or a.numeric or a.strings), 'choose exact-math with reference'
            exprs=['num_mod(nil,2)','num_mod(2,nil)','num_pow(nil,2)','num_pow(2,nil)',
                   'num_mod(bigint(7),bigint(0))','num_mod(rational(1,2),2)','num_mod(decimal(7),2)',
                   'num_pow(bigint(2),-1)','num_pow(bigint(2),bigint("4294967296"))',
                   'num_pow(rational(1,2),0.5)','num_pow(decimal(2),0.5)',
                   'let a=map{__add__:num_mod};a+2','let a=map{__add__:num_pow};a+2']
            cases=[(e,e+';',False) for e in exprs]
            cases += [('valid-exact-math','print num_mod(bigint(17),bigint(5));print num_pow(bigint(2),10);print num_pow(rational(1,2),-2);print num_pow(decimal(2),3);print num_pow(bigint(4),0.5);print num_mod(2,0);print num_pow(-1,0.5);',True),
                      ('caught-exact-math','print try num_mod(bigint(7),bigint(0)) catch memory_get(1);print try num_pow(bigint(2),-1) catch memory_get(1);',True),
                      ('mod-arity','fn call(f){return f();}call(num_mod);',False),
                      ('pow-arity','fn call(f){return f(2);}call(num_pow);',False),
                      ('pow-desugared','fn power(a,b){return a ** b;}print power(bigint(2),-1);',False)]
        if a.exact_math:
            cases=[(label,'fn invoke(f,a,b){return f(a,b);}'+source.replace('num_mod(', 'invoke(num_mod,').replace('num_pow(', 'invoke(num_pow,'),valid) for label,source,valid in cases]
        if a.constructors:
            assert a.reference and not (a.calls or a.numeric or a.strings or a.exact_math), 'choose constructors with reference'
            exprs = ['call1(bigint,nil)', 'call1(bigint,"bad")', 'call1(bigint,0.5)', 'call1(bigint,rational(1,2))', 'call1(bigint,num_mod(2,0))', 'call1(decimal,nil)', 'call1(decimal,"bad")', 'call1(decimal,num_exp(1000))', 'call1(rational,nil)', 'call1(rational,"bad")', 'call1(rational,"1/0")', 'call1(rational,num_mod(2,0))', 'call2(rational,1,0)', 'call2(rational,0.5,2)', 'call2(rational,1,0.5)', 'let a=map{__add__:rational};a+2']
            cases = [(e, e+';', False) for e in exprs]
            cases += [('bigint-arity', 'fn call(f){return f();}call(bigint);', False),
                      ('rational-arity', 'fn call(f){return f(1,2,3);}call(rational);', False),
                      ('valid-constructors', 'print bigint("9007199254740993");print decimal(rational(1,2));print rational(decimal("0.125"));print rational(2,4);print rational(4,2);', True),
                      ('caught-constructors', 'print try call1(bigint,0.5) catch memory_get(1);print try call1(decimal,"bad") catch memory_get(1);print try call1(rational,"1/0") catch memory_get(1);print try call2(rational,1,0) catch memory_get(1);', True)]
            cases = [(label, 'fn call1(f,a){return f(a);}fn call2(f,a,b){return f(a,b);}'+source, valid) for label,source,valid in cases]
        for label, source, valid in cases[a.start:a.start + a.count]:
            if a.reference:
                expected = run(prefix + 'let c=V.execution_context(true);V.run_seeded(F.frontend_compile("memory_alloc(7);").chunk,map{},c);'
                               + 'let r=V.run_seeded(F.frontend_compile(' + json.dumps(source) + ').chunk,map{},c);'
                               + 'V.close_execution(c);for value in r.output {print value;}', a.reference.resolve())
            else:
                expected = run(source)
            assert (expected['exit'] == 0) == valid, (label, expected)
            for mode in ['plain', 'optimized', 'pbcs']:
                for bounded in [False, True]:
                    text = prefix + 'let c=V.execution_context(true);'
                    text += 'V.run_seeded(F.frontend_compile("memory_alloc(7);").chunk,map{},c);'
                    text += 'let r=F.frontend_compile(' + json.dumps(source) + ');assert(r.ok,"compile");let code=r.chunk;'
                    if mode == 'optimized':
                        text += 'code=O.optimize_safe(code).chunk;'
                    if mode == 'pbcs':
                        text += 'code=P.deserialize_chunk(P.serialize_chunk(code));'
                    text += 'let r=V.' + ('run_seeded_bounded(code,map{},nil,c)' if bounded else 'run_seeded(code,map{},c)') + ';'
                    text += 'V.close_execution(c);for value in r.output {print value;}'
                    actual = run(text)
                    record(label + '/' + mode + '/' + str(bounded), actual == expected,
                           expected=expected, actual=actual)
            if not valid:
                # Fault injection affects only private cleanup metadata. Preserve
                # the original host diagnostic when that cleanup also fails.
                text = prefix + 'let c=V.execution_context(true);V.run_seeded(F.frontend_compile("memory_alloc(7);").chunk,map{},c);'
                text += 'c.memory.reclaimed=map{};V.run_seeded(F.frontend_compile(' + json.dumps(source) + ').chunk,map{},c);'
                actual = run(text)
                record(label + '/secondary-close-fault', actual == expected, expected=expected, actual=actual)


if __name__ == '__main__':
    main()
