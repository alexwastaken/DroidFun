#!/usr/bin/env python3
"""Ubuntu-only native permission fixtures. Never runs on a user's desktop.

Rules are evaluated against umockdev USB parents. ACL checks use an isolated
mount namespace, an ACL-capable character node and synthetic seat metadata, not a real login.
The UI test drives the installed app through WebKit's native automation API.
"""
import base64, errno, hashlib, json, os, pathlib, pwd, re, signal, socket, stat, subprocess, sys, tempfile, time, urllib.request
P = pathlib.Path
HELPER = '/usr/lib/droidfun/setup-robot-access'
RULE = P('/usr/lib/udev/rules.d/70-droidfun.rules')
POLICY_RULE = P('/etc/polkit-1/rules.d/00-droidfun-ci-only.rules')

def run(args, **kw):
    result = subprocess.run(args, text=True, capture_output=True, timeout=40, **kw)
    if result.returncode != 0:
        raise RuntimeError(f'{args[0]} exited {result.returncode}: {result.stdout} {result.stderr}')
    return result.stdout

def rules():
    import gi
    gi.require_version('UMockdev', '1.0')
    from gi.repository import UMockdev
    results = []
    for vid, pid, name, expected in [('2e8a','0003','ttyACM0',True), ('1234','0003','ttyACM0',False), ('2e8a','000a','ttyACM0',False), ('2e8a','0003','ttyUSB0',False)]:
        bed = UMockdev.Testbed.new()
        parent = bed.add_device('usb', '1-1', None, ['idVendor',vid,'idProduct',pid], ['DEVTYPE','usb_device'])
        device = bed.add_device('tty', name, parent, ['dev','166:0'], ['DEVNAME','/dev/'+name,'MAJOR','166','MINOR','0'])
        r = subprocess.run(['udevadm','test','--action=add',device], capture_output=True, text=True, timeout=20)
        text = r.stdout + r.stderr
        assert r.returncode == 0, text
        tagged = any('uaccess' in line for line in text.splitlines() if line.startswith(('TAGS=', 'CURRENT_TAGS=')))
        assert tagged == expected, text
        results.append({'vid':vid,'pid':pid,'ttyKind':name,'uaccess':tagged})
        bed = None
    print(json.dumps({'ruleMatching':'passed','cases':results}))

def acl():
    assert os.getuid() == 0
    user = pwd.getpwnam('nobody'); uid, gid = user.pw_uid, user.pw_gid
    # devpts does not support ACLs. Use a real character node on ext4/tmpfs,
    # like a normal /dev serial node; the null driver supplies harmless I/O.
    # No serial protocol or actual logind session is simulated by this check.
    with tempfile.TemporaryDirectory() as tmp:
        node=P(tmp)/'char-node';os.mknod(node,stat.S_IFCHR|0o660,os.makedev(1,3));os.chmod(node,0o660)
        run(['mount','--bind',str(node),'/dev/tty'])
        seats=P(tmp)/'seats';seats.mkdir()
        seat=seats/'seat0';seat.write_text(f'ACTIVE=droidfun-fixture\nACTIVE_UID={uid}\n')
        run(['mount','--bind',str(seats),'/run/systemd/seats'])
        def opens(who):
            code='import os; f=os.open("/dev/tty",os.O_RDWR|os.O_NOCTTY);os.close(f)'
            r = subprocess.run(['setpriv',f'--reuid={who}',f'--regid={gid}','--clear-groups','python3','-c',code],capture_output=True)
            return r.returncode == 0
        assert not opens(uid), 'Fixture must reproduce kernel permission denial'
        run(['udevadm','test-builtin','uaccess','/sys/devices/virtual/tty/tty'])
        assert opens(uid), 'Active local user did not gain real kernel read/write access'
        assert not opens(1), 'An unrelated user must remain denied'
        seat.write_text('ACTIVE=droidfun-other\nACTIVE_UID=1\n')
        run(['udevadm','test-builtin','uaccess','/sys/devices/virtual/tty/tty'])
        assert not opens(uid) and opens(1), 'Seat switch must revoke the old user ACL'
        print(json.dumps({'aclMechanism':'passed','initialKernelDenial':True,'activeSeatUserCanOpen':True,'unrelatedUserDenied':True,'seatSwitchRevokesOldUser':True,'fixture':'real character node with harmless null-driver I/O; private mount namespace and synthetic seat metadata, not a serial transport or live desktop session'}))

class Driver:
    @staticmethod
    def app_pids():
        result=set()
        for item in P('/proc').iterdir():
            if not item.name.isdigit(): continue
            try:
                if item.stat().st_uid != os.getuid(): continue
                executable=os.readlink(item/'exe')
                args=(item/'cmdline').read_bytes().split(b'\0')
                native=executable == '/usr/bin/deskbot'
                translated=executable == '/mnt/lima-rosetta/rosetta' and b'/usr/bin/deskbot' in args
                if native or translated: result.add(int(item.name))
            except (FileNotFoundError, PermissionError, ProcessLookupError): pass
        return result
    def __init__(self):
        self.before=self.app_pids()
        s=socket.socket();s.bind(('127.0.0.1',0));self.port=s.getsockname()[1];s.close()
        self.log=open(P(os.environ['DESKBOT_ARTIFACT_DIR'])/'linux-webdriver.txt','w')
        self.child=subprocess.Popen(['WebKitWebDriver',f'--port={self.port}'],env={**os.environ,'TAURI_WEBVIEW_AUTOMATION':'true'},stdout=self.log,stderr=self.log)
        self.session=None
        for _ in range(100):
            try:
                self.request('GET','/status');break
            except Exception:time.sleep(.1)
        try:
            self.session=self.request('POST','/session',{'capabilities':{'alwaysMatch':{'webkitgtk:browserOptions':{'binary':'/usr/bin/deskbot','args':[]}}}})['sessionId']
            assert len(self.app_pids()-self.before) == 1, 'Expected exactly one newly launched installed app'
        except Exception:
            self.close()
            raise
    def request(self, method, path, data=None):
        req=urllib.request.Request(f'http://127.0.0.1:{self.port}'+path,data=None if data is None else json.dumps(data).encode(),headers={'Content-Type':'application/json'},method=method)
        with urllib.request.urlopen(req,timeout=140) as r:value=json.load(r)['value']
        if isinstance(value,dict) and 'error' in value:raise RuntimeError(value)
        return value
    def js(self, script):return self.request('POST',f'/session/{self.session}/execute/sync',{'script':script,'args':[]})
    def wait(self, expression, seconds=30):
        deadline=time.monotonic()+seconds
        while time.monotonic()<deadline:
            if self.js('return Boolean('+expression+');'):return
            time.sleep(.2)
        raise AssertionError('Timed out waiting for: '+expression+'\n'+str(self.js('return document.body.innerText')))
    def capture(self,name):
        encoded=self.request('GET',f'/session/{self.session}/screenshot')
        (P(os.environ['DESKBOT_ARTIFACT_DIR'])/name).write_bytes(base64.b64decode(encoded))
    def close(self):
        if self.session:
            try:self.request('DELETE',f'/session/{self.session}')
            except Exception:pass
        self.child.terminate();self.child.wait(timeout=10);self.log.close()
        # Only the exact installed executable, newly started by this test.
        for pid in self.app_pids()-self.before:
            try: os.kill(pid,signal.SIGTERM)
            except ProcessLookupError: pass

def ui():
    driver=Driver()
    rule_installed_before = RULE.read_bytes()
    themes_verified = []
    def choose_bot(model, capture_chooser=False):
        driver.wait('window.__TAURI_INTERNALS__ && document.querySelector(".app-shell, .bot-picker")')
        if not driver.js('return Boolean(document.querySelector(".bot-picker"))'):
            if driver.js('return Boolean(document.querySelector(".welcome-explore button"))'):
                driver.js('document.querySelector(".welcome-explore button").click();')
            else:
                driver.js('document.querySelector(".header-settings").click();')
                driver.wait('document.querySelector(".settings-tabs")')
                driver.js('Array.from(document.querySelectorAll(".settings-tabs button")).find(b => b.textContent.trim() === "Droid").click();')
                driver.js('Array.from(document.querySelectorAll(".settings-editor button")).find(b => b.textContent.trim() === "Change model").click();')
        driver.wait('document.querySelectorAll(".bot-choice").length === 4 && !document.documentElement.hasAttribute("data-bot")')
        if capture_chooser: driver.capture('linux-bot-chooser.png')
        selector = '.bot-choice[data-bot="'+model+'"]'
        driver.js('document.querySelector('+json.dumps(selector)+').click();')
        driver.wait('!document.querySelector(".bot-picker") && document.querySelector(".robot-access-setup button") && document.documentElement.dataset.bot === '+json.dumps(model))
        # Theme and allowed controls must follow the real persisted bot choice,
        # not a frontend-only skin or the former saved branding preference.
        driver.js('window.__droidfunSmokeModel = null; window.__droidfunSmokeSnapshot = null; window.__TAURI_INTERNALS__.invoke("get_state").then(s => { window.__droidfunSmokeSnapshot = s; window.__droidfunSmokeModel = s.settings.preferences.botModel; });')
        driver.wait('window.__droidfunSmokeModel === '+json.dumps(model))
        assert driver.js('return document.documentElement.dataset.branding') == 'droidfun'
        driver.js('document.querySelector(".add-widget-controls button").click();')
        widget_selector = json.dumps('select[aria-label="New widget type"]')
        driver.wait('document.querySelector('+widget_selector+')')
        options = set(driver.js('return Array.from(document.querySelector('+widget_selector+').options).map(o => o.value);'))
        if model == 'claw':
            assert options == {'codex', 'claude'}, options
        else:
            assert not options & {'codex', 'claude'} and {'text', 'face', 'clock', 'weather', 'stock'} <= options, options
        driver.js('Array.from(document.querySelectorAll(".add-widget-controls button")).find(b => b.textContent.trim() === "Cancel").click();')
        driver.wait('!document.querySelector('+widget_selector+')')
        driver.wait('!document.querySelector(".robot-access-setup button").disabled')
        driver.js('document.querySelector(".robot-access-setup").scrollIntoView({block:"center"});')
        driver.capture('linux-usb-'+model+'.png')
        themes_verified.append(model)
    try:
        # A fresh customer sees a plug-in welcome. With no physical droid on
        # CI, enter preview through the real Explore screens control.
        driver.wait('document.querySelector(".welcome-shell") && document.querySelector("#robot-connection")')
        assert not driver.js('return Boolean(document.querySelector(".bot-picker"))')
        driver.capture('linux-welcome.png')
        choose_bot('meep', capture_chooser=True)
        # Headless CI has no active physical desktop. Explicit test-only polkit
        # authorization exercises real pkexec/helper IPC without a password.
        # Production policy defaults are separately checked; this rule is never shipped.
        user=pwd.getpwuid(os.getuid()).pw_name
        auth=f'polkit.addRule(function(action, subject) {{ if (action.id === "io.droidfun.setup-robot-access" && subject.user === {json.dumps(user)}) return polkit.Result.YES; }});\n'
        run(['sudo','tee',str(POLICY_RULE)],input=auth)
        run(['sudo','chmod','0644',str(POLICY_RULE)])
        time.sleep(2)
        # The test grant must not authorize another command through the same
        # interpreter. exec.argv1 is part of the real production policy.
        wrong=subprocess.run(['/usr/bin/pkexec','--disable-internal-agent','/usr/bin/sh','-c','exit 0'],capture_output=True,timeout=10)
        assert wrong.returncode == 127, 'Scoped robot action authorized a different shell command'
        extra=subprocess.run(['/usr/bin/pkexec','--disable-internal-agent','/usr/bin/sh',HELPER,'unexpected'],capture_output=True,timeout=10)
        assert extra.returncode == 2, 'Fixed helper must reject extra arguments after authorization'
        run(['sudo','rm',str(RULE)])
        driver.js('document.querySelector(".robot-access-setup button").click();')
        driver.wait('document.querySelector(".robot-access-setup [role=status]")',seconds=60)
        assert RULE.read_bytes() == rule_installed_before, 'Real helper did not restore missing installed rule'
        driver.capture('linux-usb-repair-passed.png')
        run(['sudo','rm',str(POLICY_RULE)]);time.sleep(2)
        # With no active desktop/agent, the real production policy denies it.
        driver.js('document.querySelector(".robot-access-setup button").click();')
        driver.wait('document.querySelector(".robot-access-setup [role=alert]")',seconds=30)
        assert 'not granted' in driver.js('return document.querySelector(".robot-access-setup [role=alert]").innerText')
        assert RULE.read_bytes() == rule_installed_before
        choose_bot('claw')
        # Fresh Claw defaults have no enabled usage widgets. Its caption must
        # describe the idle display rather than a nonexistent cycling countdown.
        assert driver.js('return window.__droidfunSmokeSnapshot.totalSec') == 0
        assert not driver.js('return Array.from(document.querySelectorAll(".widget-row .switch input")).some(input => input.checked)')
        driver.wait('document.querySelector(".screen-caption h2").textContent === "Idle display"')
        caption = driver.js('return document.querySelector(".screen-caption .caption-sub").textContent')
        assert caption == 'Enable a Claude or Codex widget to begin', caption
        assert not re.search(r'\bCycling\b|\d+s left', caption), caption
        choose_bot('boo')
        choose_bot('nimbus')
        choose_bot('meep')
        print(json.dumps({'nativeButton':'passed','plugInWelcome':'passed','optionalModelChooser':'passed','botSelectionRealIpc':True,'installedPackage':'/usr/bin/deskbot','installedExecutableSha256':hashlib.sha256(P('/usr/bin/deskbot').read_bytes()).hexdigest(),'realIpcPkexecAndFixedHelper':True,'missingRuleRestored':True,'alternateShellCommandDenied':True,'extraHelperArgumentsRejected':True,'deniedAuthorizationShowsError':True,'automaticBotThemes':themes_verified,'botSpecificWidgetOptions':'passed','clawIdleCaption':'passed','authorizationFixture':'temporary action+caller-specific polkit YES rule; removed before real policy denial check','humanGraphicalPasswordDialog':'not_verified','physicalRobot':'not_available'}))
    finally:
        subprocess.run(['sudo','rm','-f',str(POLICY_RULE)],check=True)
        driver.close()

if __name__ == '__main__':
    assert sys.platform == 'linux'
    {'--rules':rules,'--acl':acl,'--ui':ui}[sys.argv[1]]()
