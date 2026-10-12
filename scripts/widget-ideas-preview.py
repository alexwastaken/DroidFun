#!/usr/bin/env python3
"""Render proposed OLED layouts. Documentation only; never contacts the app/robot.

Requires Pillow. Font bytes are from font8x8 0.3.1, as used by core.rs;
their MIT notice is in docs/images/widget-ideas/FONT-LICENSE.txt.
The companion is downsampled from the firmware's shared happy-face bitmap
with the same 2x2 OR sampling and crop coordinates used by core::Frame.
"""
import hashlib
import json
import math
import pathlib
import re

from PIL import Image, ImageDraw, ImageFont

ROOT = pathlib.Path(__file__).resolve().parents[1]
OUT = ROOT / 'docs/images/widget-ideas'
FONT = json.loads((OUT / 'font8x8-basic.json').read_text())
FACE_SOURCE = ROOT / 'firmware/deskbot_firmware/fallback_face.h'
FACE = bytes(map(int, re.findall(r'\d+', FACE_SOURCE.read_text().split('{', 1)[1].split('}', 1)[0])))
assert len(FACE) == 1024

# Every string below is intentional display copy, not actual account/device data.
WIDGETS = [
    dict(name='Desk clock', title='CLOCK', context='LOCAL PDT', value='12:34', status='PM', footer='MON OCT 12'),
    dict(name='World clocks', title='WORLD TIME', context='TOKYO', value='04:34', status='JST', footer='TUE OCT 13'),
    dict(name='Pomodoro', title='POMODORO', context='FOCUS 3/4', value='18:42', progress=.252, footer='BREAK: 5 MIN'),
    dict(name='Meeting countdown', title='MEETING', context='TEAM SYNC', value='12 MIN', status='STARTS AT 13:00', footer='30 MIN / ONLINE'),
    dict(name='Event countdown', title='COUNTDOWN', context='VACATION', value='12 DAYS', status='SAT / 09:00', footer='TRIP: OCT 24'),
    dict(name='Stopwatch', title='STOPWATCH', context='LAP 03', value='04:28.6', status='LAST LAP 01:12', footer='RUNNING'),
    dict(name='Alarm', title='ALARM', context='NEXT ALARM', value='07:30', status='AM / WEEKDAYS', footer='VISUAL REMINDER'),
    dict(name='Stretch reminder', title='STRETCH', context='BREAK TIME', lines=['STAND & STRETCH', 'FOR 60 SECONDS'], footer='NEXT: 30 MIN'),
    dict(name='Water reminder', title='WATER', context='DRINK NOW', value='4 / 8', progress=.5, footer='GLASSES TODAY'),
    dict(name='Screen break', title='EYE BREAK', context='REST EYES', value='00:20', status='LOOK 20 FT AWAY', footer='BREAK EVERY 20M'),
    dict(name='Tiny to-do list', title='TO-DO', context='NEXT TASK', lines=['REVIEW THE', 'DESIGN MOCKUPS'], footer='3 / 5 COMPLETE'),
    dict(name='Habit streak', title='HABITS', context='MORNING', value='12 DAYS', status='STREAK', footer='TODAY: 2/3 DONE'),
    dict(name='Daily goal', title='GOAL', context='DAILY GOAL', value='6 / 10', progress=.6, footer='TASKS COMPLETE'),
    dict(name='Custom note', title='NOTE', context='FOR YOU', lines=["YOU'VE GOT THIS", 'ONE STEP AT'], footer='A TIME.'),
    dict(name='Computer CPU', title='CPU', context='HOST PC', value='23%', chart=[4,12,26,48,31,78,96,51,39,61,30,23], footer='LAST 60 SECONDS'),
    dict(name='Computer memory', title='MEMORY', context='HOST PC', value='12.4GB', progress=12.4/32, footer='OF 32 GB / 39%'),
    dict(name='Disk space', title='DISK', context='MAIN DRIVE', value='214 GB', progress=214/512, footer='FREE OF 512 GB'),
    dict(name='Laptop battery', title='BATTERY', context='LAPTOP', value='78%', progress=.78, footer='CHARGING / AC'),
    dict(name='Network activity', title='NETWORK', context='LIVE', lines=['DOWN 12.4 MB/S', 'UP 0.8 MB/S'], chart=[3,6,4,8,10,6,9,12,8,11,10,13], footer='TOTAL / 60 SEC'),
    dict(name='Connection health', title='CONNECTION', context='INTERNET', value='ONLINE', status='PING 18 MS', footer='CHECKED 5S AGO'),
    dict(name='Download progress', title='DOWNLOAD', context='FILE 1/2', value='63%', progress=.63, footer='400MB / 2M LEFT'),
    dict(name='Git status', title='GIT', context='DROIDFUN', value='MAIN', status='3 MOD / 1 NEW', footer='STAGED 2 / 4'),
    dict(name='Build/test status', title='BUILD', context='TEST SUITE', value='PASS', status='29 / 29 PASSED', footer='TIME: 8.2 SEC'),
    dict(name='Service health', title='SERVICES', context='LOCAL', value='3 / 3', status='DB WEB CACHE', footer='CHECKED 10S AGO'),
    dict(name='Now playing', title='MUSIC', context='PLAYING', lines=['MIDNIGHT CITY', 'M83'], progress=72/243, footer='01:12 / 04:03'),
    dict(name='Rain outlook', title='RAIN', context='NEXT HOUR', value='70%', status='AT 14:00 / 2MM', footer='BRING UMBRELLA'),
    dict(name='Air quality', title='AIR', context='US AQI', value='42 AQI', status='GOOD', footer='UPDATED 5M AGO'),
    dict(name='UV reminder', title='UV INDEX', context='TODAY', value='6 HIGH', status='PEAK AT 13:00', footer='TAKE SHADE'),
    dict(name='Sunrise/sunset', title='SUN', context='SUNSET', value='18:21', status='IN 3H 12M', footer='NEXT DAWN 06:52'),
    dict(name='Inbox count', title='INBOX', context='WORK MAIL', value='7 NEW', status='UNREAD MESSAGES', footer='SYNCED 1M AGO'),
]


def text(image, value, x, y, scale=1, limit=15):
    value = value.upper()
    assert len(value) <= limit, f'Display copy would be truncated: {value}'
    assert x + len(value) * 8 * scale <= 128 and y + 8 * scale <= 64
    for i, char in enumerate(value):
        assert 32 <= ord(char) < 127, f'Unsupported glyph: {char}'
        for row, bits in enumerate(FONT[ord(char)]):
            for col in range(8):
                if bits >> col & 1:
                    for dy in range(scale):
                        for dx in range(scale):
                            image.putpixel((x + (i * 8 + col) * scale + dx, y + row * scale + dy), 255)


def companion(image):
    for y in range(25):
        for x in range(37):
            lit = any(FACE[(sy // 8) * 128 + sx] >> (sy % 8) & 1
                      for dy in range(2) for dx in range(2)
                      for sx, sy in [(28 + x * 2 + dx, 8 + y * 2 + dy)])
            if lit:
                image.putpixel((88 + x, y), 255)


def render(widget):
    image = Image.new('L', (128, 64), 0)
    text(image, widget['title'], 4, 2, limit=10)
    text(image, widget['context'], 4, 14, limit=10)
    companion(image)
    if 'value' in widget:
        text(image, widget['value'], 4, 28, scale=2, limit=7)
    else:
        assert len(widget['lines']) == 2
        for line, y in zip(widget['lines'], [28, 39]):
            text(image, line, 4, y)
    if 'status' in widget:
        assert 'progress' not in widget and 'chart' not in widget
        text(image, widget['status'], 4, 45)
    draw = ImageDraw.Draw(image)
    if 'progress' in widget:
        # Outlined track distinguishes 0% from a missing bar.
        draw.rectangle((4, 49, 123, 52), outline=255)
        filled = round(widget['progress'] * 116)
        if filled:
            draw.rectangle((6, 50, 5 + filled, 51), fill=255)
    if 'chart' in widget:
        series = widget['chart']
        ceiling = 100 if widget['title'] == 'CPU' else max(series)
        points = [(4 + round(i * 119 / (len(series) - 1)),
                   52 - round(value / ceiling * 5)) for i, value in enumerate(series)]
        draw.line(points, fill=255, width=1)
    text(image, widget['footer'], 4, 55)
    assert set(image.tobytes()) == {0, 255}, 'OLED previews must be one-bit monochrome'
    # Data/header pixels must not intrude on the shared companion region.
    face_only = Image.new('L', (128, 64), 0)
    companion(face_only)
    assert image.crop((88, 0, 125, 25)).tobytes() == face_only.crop((88, 0, 125, 25)).tobytes()
    return image


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    cells = []
    manifest = []
    sections = ['## Exact screen proposals', '',
                'Each image below is an **illustrative layout with fictional sample values**, not a live reading. The five implemented widgets are noted above; these images remain design examples. The screen is exactly **128×64 monochrome pixels**, shown at 4× nearest-neighbor scale for readability. The companion uses the app’s original happy-face bitmap at the existing corner size; in the eventual widget it would smile/blink using the shared animation setting.', '',
                'Header and context occupy the left of the top 26 rows. The same bot remains at the upper right. Main content starts at row 28; detail/progress/history uses the space below it, and the footer starts at row 55. There is no scrolling or hidden text in these samples. Long user-entered titles/messages would be shortened or split into additional cards at implementation time.', '',
                '![All 30 proposed OLED screens](docs/images/widget-ideas/overview.png)', '']
    for number, widget in enumerate(WIDGETS, 1):
        slug = re.sub(r'[^a-z0-9]+', '-', widget['name'].lower()).strip('-')
        stem = f'{number:02d}-{slug}'
        raw = render(widget)
        raw.save(OUT / (stem + '-128x64.png'))
        enlarged = raw.resize((512, 256), Image.Resampling.NEAREST)
        enlarged.save(OUT / (stem + '.png'))
        cells.append((number, widget['name'], enlarged))
        lines = [widget['title'], widget['context']] + ([widget['value']] if 'value' in widget else widget['lines'])
        if 'status' in widget:
            lines.append(widget['status'])
        lines.append(widget['footer'])
        description = ' · '.join(f'`{line}`' for line in lines)
        sections += [f'### {number}. {widget["name"]}', '',
                     f'![{widget["name"]}: ' + '; '.join(lines) + f'](docs/images/widget-ideas/{stem}.png)', '',
                     f'**Exact text:** {description}.', '']
        if 'progress' in widget:
            label = 'free drive space' if widget['title'] == 'DISK' else 'focus interval elapsed' if widget['title'] == 'POMODORO' else 'playback elapsed' if widget['title'] == 'MUSIC' else 'completed / total or capacity'
            sections += [f'**Graphic:** outlined progress bar, {widget["progress"] * 100:.1f}% filled ({label}).', '']
        if 'chart' in widget:
            label = 'CPU load, fixed 0–100% scale' if widget['title'] == 'CPU' else 'combined upload/download rate, scale to the displayed history'
            sections += [f'**Graphic:** 60-second sparkline ({label}).', '']
        sections += [f'[Original 128×64 PNG](docs/images/widget-ideas/{stem}-128x64.png)', '']
        manifest.append({**widget, 'number': number, 'rawImage': stem + '-128x64.png',
                         'documentImage': stem + '.png', 'pixelSha256': hashlib.sha256(raw.tobytes()).hexdigest()})
    columns, cell_w, cell_h, margin, top = 3, 552, 312, 24, 104
    sheet = Image.new('RGB', (columns * cell_w + margin * 2,
                              math.ceil(len(cells) / columns) * cell_h + top + margin), '#eff5f1')
    draw = ImageDraw.Draw(sheet)
    title_font = ImageFont.load_default(size=32)
    label_font = ImageFont.load_default(size=21)
    draw.text((margin, 18), 'DESKBOT / 30 WIDGET SCREEN PROPOSALS', fill='#123c31', font=title_font)
    draw.text((margin, 62), 'Fictional samples / 128 x 64 pixels / same companion face on every screen', fill='#42635a', font=label_font)
    for i, (number, name, image) in enumerate(cells):
        x, y = margin + (i % columns) * cell_w, top + (i // columns) * cell_h
        draw.text((x, y), f'{number:02d}. {name}', fill='#123c31', font=label_font)
        sheet.paste(image.convert('RGB'), (x, y + 32))
    sheet.save(OUT / 'overview.png')
    (OUT / 'manifest.json').write_text(json.dumps({'sampleData': True, 'dimensions': [128, 64],
        'displayScale': 4, 'faceSourceSha256': hashlib.sha256(FACE_SOURCE.read_bytes()).hexdigest(),
        'fontSource': 'font8x8 0.3.1 BASIC_LEGACY (same BASIC_FONTS glyphs as app)', 'widgets': manifest}, indent=2) + '\n')
    document = ROOT / 'WIDGET_IDEAS.md'
    previous = document.read_text().split('\n## Exact screen proposals', 1)[0].rstrip()
    document.write_text(previous + '\n\n' + '\n'.join(sections).rstrip() + '\n')
    print(json.dumps({'widgets': len(WIDGETS), 'rawScreens': len(cells), 'enlargedScreens': len(cells),
                      'document': str(document), 'overview': str(OUT / 'overview.png')}))


if __name__ == '__main__':
    main()
