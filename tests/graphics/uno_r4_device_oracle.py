"""Generate a native-PBLang oracle using the same compiled firmware source."""
from pathlib import Path
import sys
adapter = '''let cells=[];for i 640 {array_push(cells,0);}let clock_ms=0;let input=[];let output="";let words=[0,0,0];
fn pb_get(i) {return cells[i];}
fn pb_set(i,v) {cells[i]=v;return 0;}
fn pb_millis() {return clock_ms;}
fn pb_serial_begin() {return 0;}
fn pb_available() {return len(input);}
fn pb_read() {if len(input)==0 {return -1;}let c=input[0];input=slice(input,1,len(input));return c;}
fn pb_writable() {return 64;}
fn pb_put(v) {output=output+strfromcharcode(v);return 1;}
fn pb_begin() {return 1;}
fn pb_clear() {words=[0,0,0];return 0;}
fn pb_render(a,b,c) {words=[a,b,c];return 0;}
'''
scenario = '''
fn feed(text) {input=str_to_bytes(text);for i 100 {pb_loop();}}
fn record(label) {print label+"\\t"+str_trim(output);output="";}
pb_setup();feed("HELLO 1234567890abcdef\\n");record("hello");
feed("PLAY 2234567890abcdef\\n");record("play");clock_ms=1000;
feed("STATUS 3234567890abcdef\\n");record("frame2");clock_ms=1500;
feed("PLAY 3234567890abcdef\\n");record("reused");
feed("PLAY 4234567890abcdef\\n");record("restart");clock_ms=1800;
feed("PLAY 4234567890abcdef\\n");record("duplicate");
feed("STOP 5234567890abcdef\\r\\n");record("stop");
pb_setup();clock_ms=4294967280;feed("PLAY a234567890abcdef\\n");record("wrapplay");
clock_ms=984;feed("STATUS b234567890abcdef\\n");record("wrapframe");
clock_ms=3984;feed("STATUS c234567890abcdef\\n");record("wrapdone");
print "uno_r4_device_native_oracle_pass";
'''
Path(sys.argv[2]).write_text(adapter + Path(sys.argv[1]).read_text() + scenario)
