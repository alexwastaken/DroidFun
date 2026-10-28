# Choose your bot and customize its screens

Prepared droids are recognized automatically. You can also explore screens without a droid by choosing **Nimbus**, **Boo**, **Meep** or **Claw**. Each remembered droid keeps its own screens and settings.
- **Boo, Meep and Nimbus:** faces, stationary or scrolling custom text, uploaded images, weather, stocks, clocks, Pomodoro and meeting timers, CPU and memory.
- **Claw:** Claude and Codex usage. Connect the appropriate account or choose sample data while setting up. Selecting Claw does not sign you into either service.

## Add your own text

1. Choose **Add a screen → Custom text**.
2. Enter a name and message. Choose **Stationary** to wrap text on screen or **Scroll across the screen** for a repeating sign.
3. Choose text size, vertical position and an optional border. Stationary text also supports horizontal alignment. Both modes can invert the display.
4. Check the pixel preview and click **Apply**. Choose its playlist duration, or preview and pin the screen to keep it playing.

Stationary text supports up to 128 printable ASCII characters and line breaks, preserving letter case. It refuses overflow with an explanation. Scrolling supports up to 256 characters; line breaks become spaces. Slow, Normal and Fast control the movement. Applying a changed scrolling message extends its duration for one pass, up to the 120-second playlist limit; pin a longer or slower message when needed. Pause stops motion. System reduced-motion preferences show a still portion instead. Scrolling requires the app running; it is excluded from wall-power saves.

## Add an image

1. Choose **Add a screen → Upload an image**.
2. Choose a PNG, JPG or still WebP file, up to 2 MB and 4 megapixels (no side over 4096 pixels).
3. Choose **Fit whole image** or **Fill screen · crop edges**. Adjust the black-and-white threshold, optional dot shading and inversion using the exact OLED preview.
4. Give it a name and click **Apply**. Its first upload enables the screen automatically. It appears in the normal playlist, where you can preview, reorder, edit, disable or remove it and change its duration.

The file is decoded and converted locally into 128 × 64 monochrome pixels. Only those pixels are saved; the original file, path and image metadata are not stored or uploaded to the website. Saved images remain available after restarting or removing the source file. To change fit or shading later, choose the original file again. Claw does not offer text or image screens.

Information screens use a simple layout with text lower on the display, a clear screen label, the main reading and its status. Weather shows WEATHER, an optional location, a large temperature and a condition or high/low reading. Information screens never include companion faces or decorative logos. Left alignment, a centered reading and display inversion remain available; dedicated face screens keep their expressions and animation. Screen-specific choices do not change the settings for other screens. A live-data preview uses the most recently available reading; editing a screen does not fetch new data or interrupt playback on the bot.

## Use your bot with a USB wall charger

For Boo, Meep and Nimbus, **Save to bot** copies your enabled faces, stationary text and image screens into the bot's storage. After a successful save, disconnect it from the computer and connect it to a USB wall charger. The bot cycles through the saved screens without the desktop app, including after power is disconnected and restored.

1. Connect your bot to the computer with a USB data cable.
2. Workshop-prepared standard and swapped-wiring factory firmware supports saved screens. If this droid reports saving is unavailable, follow the app’s Connection help or use private Workshop; do not replace factory or swapped-wiring firmware with generic recovery.
3. Enable the faces, stationary text and images you want. Up to 30 screens can be saved, with a duration of 3–120 seconds each.
4. Click **Save to bot** and wait for confirmation before unplugging.

Faces are saved as still images. Scrolling text, weather, stocks, clocks, timers, computer statistics, and Claude/Codex readings are not saved for wall-powered playback: they need the computer and, where applicable, a live data source. Claw's usage screens require the desktop app, so Claw does not offer this standalone save feature. Saving again replaces the bot's stored static playlist; editing the app's playlist alone does not replace the saved copy.

## Pause internet access while using the app

**Pause online data** is separate from saving screens to the bot. It pauses internet sources while the app remains running. USB playback, text, uploaded images, faces, local clocks and timers, and computer statistics continue to work. Live weather, stocks, and Claude/Codex usage are skipped until you turn online data back on; their saved settings and enabled switches are preserved. Sample screens may still play and remain labeled as sample data.

## Verification limits

Software tests cover text layout and overflow, saved-settings migration, profile filtering, and playlist behavior. A successful **Save to bot** response is required before treating a device's playlist as stored. Power-cycle playback on physical hardware must be checked separately; this document does not claim that hardware test has been completed.

## Settings compatibility

App 0.8.0 uses settings schema 3 for custom images and text motion. Existing screens and per-droid profiles migrate with stationary text defaults. The previous schema-2 file is kept privately as `settings.before-custom-screens.json` before its first replacement. Older app versions refuse schema 3 rather than overwriting it. Images remain subject to the existing total settings-size limit; a failed save preserves the previous file.
