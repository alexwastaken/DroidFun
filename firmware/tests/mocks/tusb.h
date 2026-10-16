#pragma once
inline bool mock_usb_suspended = false;
inline bool tud_suspended() { return mock_usb_suspended; }
