#pragma once
#include <cstdint>
struct ArduinoLEDMatrix {
 uint32_t words[3]={0,0,0};
 int begin(){return 1;}
 void clear(){for(auto& w:words)w=0;}
 void loadFrame(const uint32_t source[3]){for(int i=0;i<3;i++)words[i]=source[i];}
};
