"""Validate captured synthetic protocol records; no device I/O."""
import json, sys
records = {}
for line in open(sys.argv[1]):
    if '\t' in line:
        label, raw = line.split('\t', 1)
    else:
        raw = line
    value = json.loads(raw)
    assert len(value) == 13 and value['protocol'] == 'PBLED1'
    assert value['board'] == 'UNO_R4_WIFI' and len(value['digest']) == 64
    assert len(raw.encode()) <= 512
    records.setdefault(label, []).append(value)
def last(label): return records[label][-1]
assert last('hello')['state'] == 'idle'
assert last('play')['remaining_ms'] == 4000
assert last('frame2')['frame'] == 2 and last('frame2')['remaining_ms'] == 3000
assert last('duplicate')['remaining_ms'] == 3700
assert last('reused')['error'] == 'request-reused'
assert last('prefix')['error'] == 'command'
for label in ('overflow','partial','floodstop'):
    assert records[label][0]['error'] == 'input'
    assert last(label)['command'] == 'STOP' and last(label)['state'] == 'idle'
for label in ('uppercase','control'): assert last(label)['error'] == 'input'
assert last('wrapframe')['remaining_ms'] == 3000
assert last('wrapdone')['state'] == 'done' and last('wrapdone')['frame'] == 0
assert last('backpressure_stop')['state'] == 'idle'
assert last('matrix_failure')['error'] == 'matrix'
assert records['synchronize'][0]['error'] == 'input'
assert last('synchronize')['command'] == 'HELLO' and last('synchronize')['ok']
assert last('write_retry')['command'] == 'PLAY' and last('write_retry')['ok']
print('uno_r4_device_protocol_pass records=' + str(sum(map(len,records.values()))))
