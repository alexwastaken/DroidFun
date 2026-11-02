#include <cassert>
#include <iostream>
#include "../deskbot_firmware/deskbot_firmware.ino"
namespace DroidSaved { extern "C" { uint8_t _FS_start = 0, _FS_end = 0; } }
using Bytes = std::vector<uint8_t>;
static void packet(uint8_t kind, uint32_t id, uint32_t sequence, const Bytes &payload = {}) {
  Bytes bytes(20 + payload.size()); bytes[0]='D'; bytes[1]='B'; bytes[2]=1; bytes[3]=kind;
  put32(bytes.data()+4,id); put32(bytes.data()+8,sequence); put16(bytes.data()+12,payload.size());
  if (!payload.empty()) memcpy(bytes.data()+16,payload.data(),payload.size());
  put32(bytes.data()+16+payload.size(),crc32(bytes.data(),16+payload.size())); processPacket(bytes.data(),bytes.size());
}
static void at(uint32_t when) { mock_millis=when; loop(); }
static void eyes(bool closed) {
  assert(oled_ready && mock_oled_on && standby_blink.closed==closed);
  assert((memcmp(screen,FALLBACK_FACE,FRAME_SIZE)!=0)==closed);
  // Blinking cannot alter the mouth, forehead or any pixels outside the eyes.
  for (int y=0;y<64;++y) for(int x=0;x<128;++x) {
    bool eye=y>=12&&y<37&&((x>=29&&x<=47)||(x>=81&&x<=99));
    if(!eye) assert((screen[(y/8)*128+x]&(1u<<(y%8)))==(FALLBACK_FACE[(y/8)*128+x]&(1u<<(y%8))));
  }
}
int main() {
  Serial.connected=false; setup(); const uint32_t start=standby_blink.started;
  eyes(false); at(start+3999);eyes(false);at(start+4000);eyes(true);at(start+4199);eyes(true);at(start+4200);eyes(false);
  mock_millis=UINT32_MAX-2000;showFallback();const uint32_t wrap=mock_millis;
  at(wrap+4000);eyes(true);at(wrap+4200);eyes(false);
  Bytes saved(20+DroidSaved::RECORD_BYTES,0);memcpy(saved.data(),"DFP1",4);saved[4]=1;saved[5]=1;saved[6]=85;
  put16(saved.data()+16,8);memcpy(saved.data()+18,FALLBACK_FACE,FRAME_SIZE);DroidSaved::setGeneration(saved.data(),saved.size(),1);
  mock.files[DroidSaved::paths[0]]=saved;DroidSaved::load();standalone_active=false;playStandalone();
  const auto files=mock.files;const auto crc=DroidSaved::contentChecksum(DroidSaved::saved,DroidSaved::saved_length);
  const uint32_t wall=standby_blink.started;
  for(uint32_t n=0;n<20;++n){at(wall+n*4200+4000);eyes(true);at(wall+(n+1)*4200);eyes(false);}
  assert(mock.files==files && crc==DroidSaved::contentChecksum(DroidSaved::saved,DroidSaved::saved_length));
  // Active desktop pixels always own the display, even during a blink deadline.
  Serial.connected=true;packet(HELLO,7,1);packet(FRAME,7,2,Bytes(1024,0x33));at(mock_millis+4000);assert(screen[0]==0x33);
  mock_usb_suspended=true;loop();const auto sleep=standby_blink.started;at(sleep+4000);eyes(true);at(sleep+4200);eyes(false);
  mock_usb_suspended=false;Serial.connected=true;packet(HELLO,8,1);
  const char off[]="{\"on\":false,\"contrast\":216,\"orientation\":0}";
  packet(MSG_DISPLAY,8,2,Bytes(off,off+strlen(off)));Serial.connected=false;at(mock_millis+10000);assert(manual_off&&!mock_oled_on);
  // A custom saved image stays exactly as saved, with no eyes drawn over it.
  manual_off=false;standalone_active=false;memset(saved.data()+18,0x55,1024);DroidSaved::setGeneration(saved.data(),saved.size(),2);
  mock.files[DroidSaved::paths[0]]=saved;DroidSaved::load();playStandalone();const auto custom=mock_millis;
  for(uint32_t t : {4000u,4200u,8000u,12400u}){at(custom+t);for(auto byte:screen)assert(byte==0x55);}
  assert(mock.format_calls==0);
  std::cout<<"Wall/saved-face blink, 200ms eyelids, mouth preservation, timer wrap, flash preservation, USB handoff, suspension, screen-off and custom images: PASS\n";
}
