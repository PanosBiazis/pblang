#pragma once
#include <cstdint>
#include <deque>
#include <string>
static uint32_t fake_millis;
static uint32_t millis(){return fake_millis;}
// Renesas 1.6.0 UART inherits this default without overriding it.
struct Print { virtual int availableForWrite(){return 0;} };
struct UART:Print {
 std::deque<unsigned char> input;std::string output;
 void begin(unsigned long){}
 int available(){return input.size();}
 int read(){if(input.empty())return -1;auto b=input.front();input.pop_front();return b;}
 size_t write(uint8_t b){output+=(char)b;return 1;}
};
struct USB:UART {int availableForWrite() override{return 4;}};
#ifdef NO_USB
static UART _UART1_;
#define Serial _UART1_
#else
static USB SerialUSB;
#define Serial SerialUSB
#endif
