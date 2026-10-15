#!/usr/bin/env python3
"""Ubuntu-only native permission fixtures. Never runs on a user's desktop.

Rules are evaluated against umockdev USB parents. ACL checks use an isolated
mount namespace, a real PTY and synthetic seat metadata, not a real login.
The UI test drives the installed app through WebKit's native automation API.
"""
import base64, errno, json, os, pathlib, pty, pwd, re, socket, subprocess, sys, tempfile, time, urllib.request
P = pathlib.Path
HELPER = '/usr/lib/droidfun/setup-robot-access'
RULE = P('/usr/lib/udev/rules.d/70-droidfun.rules')
POLICY_RULE = P('/etc/polkit-1/rules.d/00-droidfun-ci-only.rules')

def run(args, **kw):
    return subprocess.run(args, text=True, capture_output=True, check=True, timeout=40, **kw).stdout

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
    master, slave = pty.openpty(); node = os.ttyname(slave)
    os.chown(node,0,0);os.chmod(node,0o660)
    # Bind only within this private namespace. The original /dev/tty is restored
    # by namespace exit; no physical robot, session file or host ACL is touched.
    run(['mount','--bind',node,'/dev/tty'])
    with tempfile.TemporaryDirectory() as tmp:
        seat = P(tmp)/'seat0'; seat.write_text(f'ACTIVE=droidfun-fixture\nACTIVE_UID={uid}\n')
        run(['mount','--bind',tmp,'/run/systemd/seats'])
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
        print(json.dumps({'aclMechanism':'passed','initialKernelDenial':True,'activeSeatUserCanOpen':True,'unrelatedUserDenied':True,'seatSwitchRevokesOldUser':True,'fixture':'real PTY; private mount namespace and synthetic seat metadata, not a live desktop session'}))
    os.close(slave);os.close(master)

class Driver:
    def __init__(self):
        s=socket.socket();s.bind(('127.0.0.1',0));self.port=s.getsockname()[1];s.close()
        self.log=open(P(os.environ['DESKBOT_ARTIFACT_DIR'])/'linux-webdriver.txt','w')
        self.child=subprocess.Popen(['WebKitWebDriver',f'--port={self.port}'],env={**os.environ,'TAURI_WEBVIEW_AUTOMATION':'true'},stdout=self.log,stderr=self.log)
        self.session=None
        for _ in range(100):
            try:
                self.request('GET','/status');break
            except Exception:time.sleep(.1)
        self.session=self.request('POST','/session',{'capabilities':{'alwaysMatch':{'webkitgtk:browserOptions':{'binary':'/usr/bin/deskbot','args':[]}}}})['sessionId']
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

def ui():
    driver=Driver()
    rule_installed_before = RULE.read_bytes()
    try:
        driver.wait('window.__TAURI_INTERNALS__ && document.querySelector(".robot-access-setup button")')
        driver.js('window.__TAURI_INTERNALS__.invoke("get_state").then(s => window.__TAURI_INTERNALS__.invoke("set_preferences",{preferences:{...s.settings.preferences,setupComplete:true}}));')
        driver.wait('!document.querySelector("[role=dialog]")')
        driver.wait('!document.querySelector(".robot-access-setup button").disabled')
        driver.js('document.querySelector(".robot-access-setup").scrollIntoView({block:"center"});')
        driver.capture('linux-usb-blue.png')
        # Headless CI has no active physical desktop. Explicit test-only polkit
        # authorization exercises real pkexec/helper IPC without a password.
        # Production policy defaults are separately checked; this rule is never shipped.
        user=pwd.getpwuid(os.getuid()).pw_name
        auth=f'polkit.addRule(function(action, subject) {{ if (action.id === "io.droidfun.setup-robot-access" && subject.user === {json.dumps(user)}) return polkit.Result.YES; }});\n'
        run(['sudo','tee',str(POLICY_RULE)],input=auth)
        run(['sudo','chmod','0644',str(POLICY_RULE)])
        time.sleep(2)
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
        driver.js('const s=document.querySelector(".branding-control select");s.value="green";s.dispatchEvent(new Event("change",{bubbles:true}));')
        driver.wait('document.documentElement.dataset.branding === "green"')
        driver.capture('linux-usb-green.png')
        print(json.dumps({'nativeButton':'passed','installedPackage':'/usr/bin/deskbot','realIpcPkexecAndFixedHelper':True,'missingRuleRestored':True,'deniedAuthorizationShowsError':True,'blueAndGreenControls':'passed','authorizationFixture':'temporary action+caller-specific polkit YES rule; removed before real policy denial check','humanGraphicalPasswordDialog':'not_verified','physicalRobot':'not_available'}))
    finally:
        subprocess.run(['sudo','rm','-f',str(POLICY_RULE)],check=True)
        driver.close()

if __name__ == '__main__':
    assert sys.platform == 'linux'
    {'--rules':rules,'--acl':acl,'--ui':ui}[sys.argv[1]]()
