"""Paired release-only JSON decoding timings; correctness is checked separately.

Each fresh process warms up once, times one parse, then emits its full decoded
value for an independent Python binary64 comparison outside the timed interval.
No source checkout, network, product pointer or transport guard is used/changed.
"""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import statistics
import struct
import subprocess
import tempfile
import time
import tomllib


def same_value(actual, expected):
    if isinstance(expected, bool) or expected is None or isinstance(expected, str):
        return type(actual) is type(expected) and actual == expected
    if isinstance(expected, (int, float)):
        return type(actual) in (int, float) and struct.pack('<d', float(actual)) == struct.pack('<d', float(expected))
    if isinstance(expected, list):
        return isinstance(actual, list) and len(actual) == len(expected) and all(same_value(a, b) for a, b in zip(actual, expected))
    return isinstance(actual, dict) and actual.keys() == expected.keys() and all(same_value(actual[k], expected[k]) for k in expected)


def fixtures():
    # Identical to cycle11 fractional_cells_65536, including query and envelope.
    frame = dict(jsonrpc='2.0', id='cost', method='tools/call', params=dict(name='ai_vector_search', arguments=dict(query=[.5]*256, items=[dict(id=str(i), vector=[.5]*256) for i in range(256)], top_k=1)))
    return [
        ('fractional_cells_65536', json.dumps(frame, separators=(',', ':'), ensure_ascii=False), 'parse'),
        ('distinct_decimals_65536', '['+','.join(str(i)+'.125' for i in range(65536))+']', 'parse'),
        ('alternating_decimals_65536', '['+','.join(['0.5','0.25']*32768)+']', 'parse'),
        ('boundary_repeat_110376', '['+','.join(['9007199254740991.0']*110376)+']', 'parse'),
        ('strict_exact_1024', '['+','.join(['0.5']*1024)+']', 'parse_exact'),
        ('dense_integer_1048575', '['+','.join(['0']*1048575)+']', 'parse'),
        ('small_decimal_object', '{"x":0.1,"y":1.25,"z":-0.0}', 'parse'),
    ]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--baseline', type=Path, required=True)
    ap.add_argument('--candidate', type=Path, required=True)
    ap.add_argument('--report', type=Path, required=True)
    ap.add_argument('--start', type=int, default=0)
    ap.add_argument('--count', type=int, default=1)
    ap.add_argument('--pairs', type=int, default=5)
    a = ap.parse_args()
    assert 1 <= a.pairs <= 10
    releases = {'baseline': a.baseline.resolve(), 'candidate': a.candidate.resolve()}
    out = dict(passed=False, scope='source-hidden parser-only milliseconds, fresh process, one warmup; RSS includes output validation serialization', pairs=a.pairs, rows=[], decoder_sha256={k:hashlib.sha256((p/'stdlib/json_decode.sapp').read_bytes()).hexdigest() for k,p in releases.items()})
    def save():
        a.report.parent.mkdir(parents=True, exist_ok=True)
        a.report.write_text(json.dumps(out, indent=2)+'\n')
    with tempfile.TemporaryDirectory(prefix='pb-json-cost-') as name:
        root = Path(name)
        def run(selected, document, method):
            manifest = '[package]\nname="json-cost"\nversion="1.0.0"\nentry="main.sapp"\n[dependencies]\nstdlib={path='+json.dumps(str(selected/'stdlib'))+'}\n'
            (root/'pblang.toml').write_text(manifest)
            dep = tomllib.loads((selected/'pblang.lock').read_text())['dependencies']['stdlib']
            (root/'pblang.lock').write_text('manifest_sha256 = "'+hashlib.sha256(manifest.encode()).hexdigest()+'"\ncontent_hash_algorithm = "pblang-package-tree-v2"\n[dependencies.stdlib]\npath = '+json.dumps(str(selected/'stdlib'))+'\nsha256 = '+json.dumps(dep['sha256'])+'\n')
            (root/'input.json').write_text(document)
            (root/'main.sapp').write_text('let D=import("stdlib:json_decode.sapp");let text=fs_read("input.json");let value=D.'+method+'(text);value=nil;let start=now_ms();value=D.'+method+'(text);print now_ms()-start;print json_stringify(value);')
            box = ['bwrap','--unshare-all','--die-with-parent']
            for d in ['/usr','/bin','/lib','/lib64']:box += ['--ro-bind',d,d]
            box += ['--proc','/proc','--dev','/dev','--tmpfs','/tmp','--ro-bind',str(selected),str(selected),'--bind',name,name,'--chdir',name]
            start = time.monotonic()
            p = subprocess.run(box+['/usr/bin/time','-f','%M','-o',str(root/'rss'),str(selected/'bin/pblang'),'run',str(root/'main.sapp')]+['--deny-'+d for d in ['net','http','exec','ffi','spawn','gfx','sqlite']],capture_output=True,text=True,timeout=30,env=os.environ|{'PBLANG_CHECK_PROFILE':'','PBLANG_SCRIPT_ARGS':''})
            assert p.returncode == 0, (p.returncode, p.stderr)
            elapsed, value = p.stdout.split('\n', 1)
            assert same_value(json.loads(value), json.loads(document)), 'independent full output comparison failed'
            return dict(parse_ms=float(elapsed),process_seconds=time.monotonic()-start,max_rss_kib=int((root/'rss').read_text().splitlines()[-1]))
        for label,document,method in fixtures()[a.start:a.start+a.count]:
            row=dict(label=label,bytes=len(document.encode()),fixture_sha256=hashlib.sha256(document.encode()).hexdigest(),method=method,samples=[])
            out['rows'].append(row);save()
            for pair in range(a.pairs):
                sample={'pair':pair}
                for key in (['baseline','candidate'] if pair%2==0 else ['candidate','baseline']):
                    sample[key]=run(releases[key],document,method)
                row['samples'].append(sample);save()
            row['summary']={k:{'median_parse_ms':statistics.median(s[k]['parse_ms'] for s in row['samples']),'p95_parse_ms':sorted(s[k]['parse_ms'] for s in row['samples'])[math.ceil(.95*a.pairs)-1],'median_process_seconds':statistics.median(s[k]['process_seconds'] for s in row['samples']),'peak_rss_kib':max(s[k]['max_rss_kib'] for s in row['samples'])} for k in releases}
            row['full_output_binary64_parity']=True;save();print(label,json.dumps(row['summary']),flush=True)
    out['passed']=True;save()


if __name__ == '__main__':main()
