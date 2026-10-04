// Compile the actual adapter, including the actual PBLang-generated header.
#include <cassert>
#include <iostream>
#include "../../examples/uno_r4_led_studio/device_adapter.ino"
int main(){
 setup();
#ifdef NO_USB
 assert(Serial.availableForWrite()==0 && pb_writable()==1);
#else
 assert(Serial.availableForWrite()==4 && pb_writable()==4);
#endif
 for(unsigned char c:std::string("HELLO 1234567890abcdef\n")){Serial.input.push_back(c);}
 for(int i=0;i<100;i++){loop();}
 assert(Serial.output.find("\"command\":\"HELLO\"")!=std::string::npos);
 assert(Serial.output.find("\"ok\":true")!=std::string::npos);
 assert(Serial.output.back()=='\n');
 std::cout<<"uno_r4_actual_adapter_hello_pass\n";
}
