# Choose your bot and customize its screens

DroidFun App asks which bot you have when you open it for the first time. Choose **Boo Bot**, **Claw Bot**, **Meep Bot**, or **Nimbus Bot**. You can use **Change bot** later. Your existing screens and settings stay saved when you switch; the app only shows the screens available for the selected bot.

- **Boo, Meep and Nimbus:** faces, custom text, weather, stocks, clocks, Pomodoro and meeting timers, CPU and memory.
- **Claw:** Claude and Codex usage. Connect the appropriate account or choose sample data while setting up. Selecting Claw does not sign you into either service.

## Add your own text

1. Add a **Custom text** screen and open its editor.
2. Enter your message, then choose small or large text, horizontal and vertical alignment, and an optional border. You can also invert the display.
3. Check the OLED preview and save. Enable the screen and choose how long it appears in your cycle.

The display is 128 × 64 pixels. Text preserves uppercase and lowercase letters and supports printable ASCII characters and line breaks, up to 128 characters. Words wrap automatically. A message that cannot fit is rejected with an explanation; reduce the font size, shorten the message, or remove the border. Text is never silently cut off to make it fit.

Information screens use a simple layout with text lower on the display, a clear screen label, the main reading and its status. Weather shows WEATHER, an optional location, a large temperature and a condition or high/low reading. Information screens never include companion faces or decorative logos. Left alignment, a centered reading and display inversion remain available; dedicated face screens keep their expressions and animation. Screen-specific choices do not change the settings for other screens. A live-data preview uses the most recently available reading; editing a screen does not fetch new data or interrupt playback on the bot.

## Use your bot with a USB wall charger

For Boo, Meep and Nimbus, **Save to bot** copies your enabled face and text screens into the bot's storage. After a successful save, disconnect it from the computer and connect it to a USB wall charger. The bot cycles through the saved screens without the desktop app, including after power is disconnected and restored.

1. Connect your bot to the computer with a USB data cable.
2. Use firmware **1.2.0 or newer**. If the app says an update is required, follow **Settings → Robot → Install firmware manually**, then reconnect.
3. Enable the face and text screens you want. Up to 30 screens can be saved, with a duration of 3–120 seconds each.
4. Click **Save to bot** and wait for confirmation before unplugging.

Faces are saved as still images. Weather, stocks, clocks, timers, computer statistics, and Claude/Codex readings are not saved for wall-powered playback: they need the computer and, where applicable, a live data source. Claw's usage screens require the desktop app, so Claw does not offer this standalone save feature. Saving again replaces the bot's stored face/text playlist; editing the app's playlist alone does not replace the saved copy.

## Pause internet access while using the app

**Pause online data** is separate from saving screens to the bot. It pauses internet sources while the app remains running. USB playback, text, faces, local clocks and timers, and computer statistics continue to work. Live weather, stocks, and Claude/Codex usage are skipped until you turn online data back on; their saved settings and enabled switches are preserved. Sample screens may still play and remain labeled as sample data.

## Verification limits

Software tests cover text layout and overflow, saved-settings migration, profile filtering, and playlist behavior. A successful **Save to bot** response is required before treating a device's playlist as stored. Power-cycle playback on physical hardware must be checked separately; this document does not claim that hardware test has been completed.
