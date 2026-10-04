// Synthetic byte/clock/matrix adapter for the actual PBLang-generated firmware.
// Never opens devices. Build with -I <generated sketch folder>.
#include <cassert>
#include <cstdint>
#include <deque>
#include <iostream>
#include <string>
static double cells[640]; static uint32_t clock_ms, words[3];
static std::deque<unsigned char> input; static std::string output;
static bool writable=true, ready=true, accept_write=true;
static double pb_get(double i){assert(i>=0 && i<640 && i==(int)i);return cells[(int)i];}
static double pb_set(double i,double v){assert(i>=0 && i<640 && i==(int)i);cells[(int)i]=v;return 0;}
static double pb_millis(){return clock_ms;}
static double pb_serial_begin(){return 0;}
static double pb_available(){return input.size();}
static double pb_read(){if(input.empty())return -1;auto c=input.front();input.pop_front();return c;}
static double pb_writable(){return writable?64:0;}
static double pb_put(double v){assert(v>=0 && v<=127);if(!accept_write)return 0;output+=(char)v;return 1;}
static double pb_begin(){return ready?1:0;}
static double pb_clear(){for(auto& w:words)w=0;return 0;}
static double pb_render(double a,double b,double c){words[0]=(uint32_t)a;words[1]=(uint32_t)b;words[2]=(uint32_t)c;return 0;}
#include "pblang_firmware.h"
static void loops(unsigned n=100){for(unsigned i=0;i<n;i++)pb_loop();}
static void feed(const std::string& s){for(unsigned char c:s)input.push_back(c);loops();}
static void reset(){input.clear();output.clear();clock_ms=0;writable=true;ready=true;accept_write=true;pb_setup();}
static void record(const char* label){assert(!output.empty() && output.back()=='\n');std::cout<<label<<"\t"<<output;output.clear();}
int main(){
 std::cout.setf(std::ios::unitbuf);
 reset();assert(cells[1]==0 && cells[3]==0 && words[0]==0);
 feed("HELLO 1234567890abcdef\n");record("hello");
 feed("PLAY 2234567890abcdef\n");record("play");assert(cells[1]==1 && cells[3]==1);
 clock_ms=1000;loops();assert(cells[3]==2);
 assert(words[0]==(uint32_t)project_word(1,0) && words[0]>0x7fffffffU);
 feed("STATUS 3234567890abcdef\n");record("frame2");
 clock_ms=1500;feed("PLAY 3234567890abcdef\n");record("reused");assert(cells[2]==0);
 feed("PLAY 4234567890abcdef\n");record("restart");assert(cells[2]==1500);
 clock_ms=1800;feed("PLAY 4234567890abcdef\n");record("duplicate");assert(cells[2]==1500);
 feed("STOP 5234567890abcdef\r\n");record("stop");assert(cells[1]==0 && words[0]==0);
 feed("PLAYX 6234567890abcdef\n");record("prefix");assert(cells[1]==0);
 feed(std::string(100,'X')+"\nSTOP 7234567890abcdef\n");record("overflow");assert(cells[1]==0);
 feed("PLAY 8");clock_ms+=501;loops();feed("234567890abcdef\nSTOP 9234567890abcdef\n");record("partial");assert(cells[1]==0);
 feed("PLAY A234567890abcdef\n");record("uppercase");assert(cells[1]==0);
 feed(std::string("PLAY ")+char(1)+"234567890abcdef\n");record("control");
 reset();clock_ms=0xfffffff0U;feed("PLAY a234567890abcdef\n");record("wrapplay");
 clock_ms=984;loops();assert(cells[3]==2);feed("STATUS b234567890abcdef\n");record("wrapframe");
 clock_ms=3984;loops();assert(cells[1]==0 && cells[4]==2 && words[0]==0);feed("STATUS c234567890abcdef\n");record("wrapdone");
 reset();writable=false;feed("PLAY d234567890abcdef\n");assert(cells[1]==1 && cells[10]>0 && output.empty());
 clock_ms=4000;loops();assert(cells[1]==0 && cells[4]==2 && words[0]==0);
 writable=true;loops();record("backpressure_old_ack");feed("STOP e234567890abcdef\n");record("backpressure_stop");
 reset();feed("PLAY f234567890abcdef\n");output.clear();
 for(unsigned i=0;i<5000;i++){input.push_back('X');}
 clock_ms=4000;pb_loop();assert(cells[1]==0 && cells[4]==2 && words[0]==0);
 loops(200);feed("\nSTOP 1234567890abcdee\n");record("floodstop");
 reset();ready=false;pb_setup();feed("PLAY 1234567890abcded\n");record("matrix_failure");assert(cells[1]==0);
 pb_render(2147483648.0,4294967295.0,0);assert(words[0]==0x80000000U && words[1]==0xffffffffU);
 reset();feed("\nHELLO 1234567890abcdef\n");record("synchronize");assert(cells[1]==0);
 reset();accept_write=false;feed("PLAY 1234567890abcdef\n");assert(cells[11]==0 && output.empty());
 clock_ms=4000;loops();assert(cells[1]==0 && words[0]==0);accept_write=true;loops();record("write_retry");
 std::cerr<<"uno_r4_device_firmware_pass\n";
}
