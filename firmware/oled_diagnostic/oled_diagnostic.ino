#include <Arduino.h>
#include <Wire.h>
#include <Adafruit_GFX.h>
#include <Adafruit_SSD1306.h>

// Independent, persistent panel test. No host app or Deskbot protocol needed.
Adafruit_SSD1306 oled(128, 64, &Wire, -1, 100000, 100000);
bool ready = false;
bool allPixels = true;

bool command(const uint8_t *bytes, size_t size) {
  Wire.beginTransmission(0x3C);
  Wire.write(0x00);
  Wire.write(bytes, size);
  return Wire.endTransmission() == 0;
}

// Set each 8-row page and column explicitly, rather than relying on
// horizontal auto-increment across the full 1,024-byte display memory.
bool displayPages() {
  const uint8_t mode[] = {0x2E, 0x20, 0x02};
  if (!command(mode, sizeof(mode))) return false;
  for (uint8_t page = 0; page < 8; ++page) {
    const uint8_t position[] = {static_cast<uint8_t>(0xB0 | page), 0x00, 0x10};
    if (!command(position, sizeof(position))) return false;
    for (uint8_t column = 0; column < 128; column += 16) {
      Wire.beginTransmission(0x3C);
      Wire.write(0x40);
      Wire.write(oled.getBuffer() + page * 128 + column, 16);
      if (Wire.endTransmission() != 0) return false;
    }
  }
  return true;
}

void setup() {
  Serial.begin(115200);
  delay(250);
  Wire.setSDA(4);
  Wire.setSCL(5);
  Wire.begin();
  Wire.setClock(100000);
  Wire.beginTransmission(0x3C);
  const int busError = Wire.endTransmission();
  ready = busError == 0 && oled.begin(SSD1306_SWITCHCAPVCC, 0x3C, false, false);
  if (!ready) return;
  oled.clearDisplay();
  oled.drawRect(0, 0, 128, 64, SSD1306_WHITE);
  oled.setTextColor(SSD1306_WHITE);
  oled.setTextSize(2);
  oled.setCursor(20, 12);
  oled.println("HELLO");
  oled.setTextSize(1);
  oled.setCursor(22, 42);
  oled.println("DESKBOT TEST");
  ready = displayPages();
  // Bypass GDDRAM to distinguish image-transfer errors from row output.
  const uint8_t allOn[] = {0x81, 0x40, 0xA5, 0xAF};
  ready = ready && command(allOn, sizeof(allOn));
}

void loop() {
  static uint32_t nextReport = 0;
  while (Serial.available()) {
    const char request = Serial.read();
    if (request == 't') {
      const uint8_t normal[] = {0xA4};
      ready = command(normal, sizeof(normal)) && displayPages();
      allPixels = false;
    } else if (request == 'a') {
      const uint8_t allOn[] = {0xA5, 0xAF};
      ready = command(allOn, sizeof(allOn));
      allPixels = true;
    } else if (request == 'c' || request == 'n') {
      const uint8_t com[] = {0xDA, static_cast<uint8_t>(request == 'c' ? 0x02 : 0x12)};
      ready = command(com, sizeof(com));
    }
  }
  if (Serial.dtr() && static_cast<int32_t>(millis() - nextReport) >= 0) {
    Serial.println(!ready ? "OLED_DIAGNOSTIC: I2C failed" : allPixels ? "OLED_DIAGNOSTIC: ALL PIXELS ON" : "OLED_DIAGNOSTIC: page-addressed text");
    nextReport = millis() + 2000;
  }
  delay(10);
}
