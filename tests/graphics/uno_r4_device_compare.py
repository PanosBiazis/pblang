"""Compare selected exact replies from native PBLang and transpiled C."""
import json,sys
c={}
for line in open(sys.argv[1]):
    if '\t' in line:
        key,value=line.split('\t',1);c[key]=json.loads(value)
count=0
for line in open(sys.argv[2]):
    if '\t' in line:
        key,value=line.split('\t',1)
        assert json.loads(value)==c[key],key
        count+=1
assert count==10,count
print('uno_r4_device_native_c_parity_pass replies='+str(count))
