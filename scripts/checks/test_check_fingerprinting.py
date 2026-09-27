#!/usr/bin/env python3
"""Tests for the fingerprinting check. The check is CI's authority to fail a
build over a read of a device, display, font, network or timing
characteristic, so its own behaviour is checked rather than assumed -- above
all that it FAILS: a clean tree passes, and a read of every category the check
covers fails.
The same read hashed under a rotating salt fails too, because FR-036a binds on
the derivation and not on what it produces.

The sources quoted in this file are Python string literals; the check reads
Rust source, script, markup and Cargo manifests, never Python, so quoting them
here is not a breach and needs no assembly trick.

Run: python3 scripts/checks/test_check_fingerprinting.py
"""
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import check_fingerprinting as check  # noqa: E402

PASSED = FAILED = 0

# Passed as `allowlist` to leave the allowlist file out of the tree entirely.
MISSING = object()


def report(name, condition):
    global PASSED, FAILED
    if condition:
        PASSED += 1
    else:
        FAILED += 1
        print(f"FAIL: {name}", file=sys.stderr)


# --- fixtures -----------------------------------------------------------------

CLEAN_RUST = (
    "#![forbid(unsafe_code)]\n"
    "use std::time::{Duration, Instant};\n"
    "\n"
    "/// Loads time out after a while; the tab model measures an interval.\n"
    "/// MachineGuid in a doc comment is not a read.\n"
    "pub fn timed_out(start: Instant) -> bool {\n"
    "    // Nor is /etc/machine-id named in a line comment.\n"
    "    /* Nor GetAdaptersAddresses in a block comment. */\n"
    "    start.elapsed() > Duration::from_secs(30)\n"
    "}\n"
    "\n"
    "pub fn draw() -> u32 {\n"
    "    let mysysinfo_cache = 7;\n"
    "    let scale = window_scale_factor();\n"
    "    mysysinfo_cache + scale\n"
    "}\n"
)

CLEAN_MANIFEST = (
    '[package]\nname = "x"\nversion = "0.0.0"\n\n'
    '[dependencies]\nserde = "1"\ngetrandom = "0.3"\n'
)


def tree(files, allowlist=""):
    """Build the files in a temporary tree and run the check over it.

    `files` maps a relative POSIX path to its content; bytes are written raw,
    so a test can plant a file the check cannot decode. `allowlist` is the
    allowlist's text, written beside the tree rather than in it, or MISSING
    to leave it absent. Returns what `check_tree` returns.
    """
    with tempfile.TemporaryDirectory() as tmp:
        base = Path(tmp)
        root = base / "tree"
        root.mkdir()
        for relative, content in files.items():
            path = root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            if isinstance(content, bytes):
                path.write_bytes(content)
            else:
                path.write_text(content, encoding="utf-8")
        allowlist_path = base / "fingerprinting-allowlist.txt"
        if isinstance(allowlist, bytes):
            allowlist_path.write_bytes(allowlist)
        elif allowlist is not MISSING:
            allowlist_path.write_text(allowlist, encoding="utf-8")
        return check.check_tree(root, allowlist_path)


def passing_tree(extra=None):
    files = {
        "crates/x/src/lib.rs": CLEAN_RUST,
        "crates/x/Cargo.toml": CLEAN_MANIFEST,
    }
    if extra:
        files.update(extra)
    return files


def with_rust(body):
    """A passing tree plus one Rust file holding `body`."""
    return passing_tree({"crates/x/src/probe.rs": body})


def mentions(problems, *fragments):
    return any(all(fragment in problem for fragment in fragments) for problem in problems)


def run_main(*arguments):
    return subprocess.run(
        [sys.executable, str(HERE / "check_fingerprinting.py"), *arguments],
        capture_output=True,
        text=True,
    )


# --- the repository itself ----------------------------------------------------

problems, read, _ = check.check_tree(check.REPO)
report("the repository passes", problems == [])
report("...having read the update client",
       "crates/evreos-platform/src/update.rs" in read)
report("...the rollout draw", "crates/evreos-platform/src/update/rollout.rs" in read)
report("...the shell", any(path.startswith("crates/evreos-shell/src/") for path in read))
report("...and the workspace manifests", "Cargo.toml" in read
       and "crates/evreos-platform/Cargo.toml" in read)

check_problems = []
entries = check.read_allowlist(check.ALLOWLIST, check_problems)
# Readable, and nothing more: the first entry lands with the read it answers
# and is proved by the repository passing above, so no case here pins the
# list to what v1 ships.
report("the committed allowlist is readable", check_problems == [])

# --- the table itself ---------------------------------------------------------

names = [name for _, name, _ in check.COMPILED]
report("every source name is unique", len(names) == len(set(names)))
report("no source name holds whitespace, which an entry could not spell",
       all(name.split() == [name] for name in names))
report("every dependency is filed under a category the sources use",
       set(check.DEPENDENCY_SOURCES.values()) <= set(check.SOURCES))
# T061's eight, as the table names them: "MAC and platform UUIDs" are split
# between the machine identifiers (the platform UUID) and the network
# category, and "processor model" is widened to its count.
report("the eight categories T061 names are all present, and no other",
       set(check.SOURCES) == {
           "machine and volume identifier",
           "MAC address or network characteristic",
           "screen geometry",
           "installed fonts",
           "timezone",
           "total memory",
           "processor model or count",
           "high-resolution timing correlator",
       })

# --- a clean tree -------------------------------------------------------------

problems, read = tree(passing_tree())[:2]
report("a clean tree passes: Instant, a scale factor, a longer identifier and "
       "comments naming sources are not reads", problems == [])
report("...reading its Rust source and its manifest",
       read == ["crates/x/Cargo.toml", "crates/x/src/lib.rs"])

# --- SOURCE: one caught case per category ------------------------------------

CAUGHT = (
    ("machine and volume identifier", "MachineGuid",
     'let id = registry.get_string("MachineGuid");\n'),
    ("machine and volume identifier", "machine-id",
     'let id = std::fs::read_to_string("/etc/machine-id");\n'),
    ("machine and volume identifier", "IOPlatformUUID",
     'let key = "IOPlatformUUID";\n'),
    ("machine and volume identifier", "uname",
     "let node = rustix::system::uname().nodename().to_owned();\n"),
    ("machine and volume identifier", "nodename",
     "let node = utsname.nodename();\n"),
    ("machine and volume identifier", "/proc/sys/kernel/hostname",
     'let host = read_to_string("/proc/sys/kernel/hostname");\n'),
    ("machine and volume identifier", "gethostid",
     "let id = unsafe { libc::gethostid() };\n"),
    ("machine and volume identifier", "boot_id",
     'let boot = read_to_string("/proc/sys/kernel/random/boot_id");\n'),
    ("machine and volume identifier", "hostName",
     "let host = NSProcessInfo::processInfo().hostName();\n"),
    ("machine and volume identifier", "SCDynamicStoreCopyComputerName",
     "let name = SCDynamicStoreCopyComputerName(None, null_mut());\n"),
    ("machine and volume identifier", "dmidecode",
     'let uuid = Command::new("dmidecode").args(["-s", "system-uuid"]).output();\n'),
    ("machine and volume identifier", "wmic",
     'let uuid = Command::new("wmic").args(["csproduct", "get", "uuid"]).output();\n'),
    ("machine and volume identifier", "ioreg",
     'let uuid = Command::new("ioreg").args(["-rd1", "-c", "IOPlatformDevice"]).output();\n'),
    ("machine and volume identifier", "getHighEntropyValues",
     'let hints = "navigator.userAgentData.getHighEntropyValues([\'model\'])";\n'),
    ("machine and volume identifier", "HW_MODEL",
     "let mib = [libc::CTL_HW, libc::HW_MODEL];\n"),
    ("machine and volume identifier", "HW_MACHINE",
     "let mib = [libc::CTL_HW, libc::HW_MACHINE];\n"),
    ("machine and volume identifier", "KERN_HOSTNAME",
     "let mib = [libc::CTL_KERN, libc::KERN_HOSTNAME];\n"),
    ("machine and volume identifier", "KERN_HOSTID",
     "let mib = [libc::CTL_KERN, libc::KERN_HOSTID];\n"),
    ("machine and volume identifier", "COMPUTERNAME",
     'let host = std::env::var("COMPUTERNAME");\n'),
    ("machine and volume identifier", "USERDOMAIN",
     'let domain = std::env::var("USERDOMAIN");\n'),
    ("machine and volume identifier", "LOGONSERVER",
     'let server = std::env::var("LOGONSERVER");\n'),
    ("machine and volume identifier", "HOSTNAME",
     'let host = std::env::var("HOSTNAME");\n'),
    ("machine and volume identifier", "GetVolumeInformation",
     "unsafe { GetVolumeInformationW(root, None, Some(&mut serial), None, None, None) };\n"),
    ("MAC address or network characteristic", "RTCPeerConnection",
     'const PROBE: &str = "const pc = new RTCPeerConnection({});";\n'),
    ("MAC address or network characteristic", "RTCPeerConnection",
     'const PROBE: &str = "const pc = new webkitRTCPeerConnection({});";\n'),
    ("MAC address or network characteristic", "RTCPeerConnection",
     'const PROBE: &str = "const pc = new mozRTCPeerConnection({});";\n'),
    ("MAC address or network characteristic", "GetAdaptersAddresses",
     "let adapters = GetAdaptersAddresses(AF_UNSPEC, 0, None, buffer, &mut size);\n"),
    ("MAC address or network characteristic", "/proc/net/arp",
     'let neighbours = read_to_string("/proc/net/arp")?;\n'),
    ("MAC address or network characteristic", "getmac",
     'let macs = Command::new("getmac").output()?;\n'),
    ("MAC address or network characteristic", "ifconfig",
     'let interfaces = Command::new("ifconfig").output()?;\n'),
    ("MAC address or network characteristic", "ipconfig",
     'let adapters = Command::new("ipconfig").arg("/all").output()?;\n'),
    ("MAC address or network characteristic", "networksetup",
     'let ports = Command::new("networksetup").arg("-listallhardwareports").output()?;\n'),
    ("MAC address or network characteristic", "/sys/class/net",
     'let mac = read("/sys/class/net/eth0/address");\n'),
    ("screen geometry", "primary_monitor",
     "let size = event_loop.primary_monitor().map(|m| m.size());\n"),
    ("screen geometry", "GetSystemMetrics",
     "let width = GetSystemMetrics(SM_CXSCREEN);\n"),
    ("screen geometry", "CGDisplay",
     "let width = CGDisplay::main().pixels_wide();\n"),
    ("screen geometry", "CGDirectDisplayID",
     "let ids: Vec<CGDirectDisplayID> = displays();\n"),
    ("screen geometry", "DisplayInformation",
     "let info = DisplayInformation::GetForCurrentView()?;\n"),
    ("installed fonts", "create_for_all_families",
     "let fonts = core_text::font_collection::create_for_all_families();\n"),
    ("installed fonts", "FontCollection::system",
     "let fonts = dwrote::FontCollection::system();\n"),
    ("installed fonts", "FontCollection::system",
     "let fonts = FontCollection::get_system(false);\n"),
    ("installed fonts", "EnumFontFamilies",
     "EnumFontFamiliesExW(dc, &logfont, Some(collect), 0, 0);\n"),
    ("installed fonts", "load_system_fonts",
     "database.load_system_fonts();\n"),
    ("installed fonts", "/Library/Fonts",
     'let dir = std::fs::read_dir("/System/Library/Fonts");\n'),
    ("installed fonts", "/usr/share/fonts",
     'let dir = std::fs::read_dir("/usr/local/share/fonts");\n'),
    ("installed fonts", "/usr/share/fonts",
     'let dir = home.join(".local/share/fonts");\n'),
    ("installed fonts", "~/.fonts",
     'let dir = std::fs::read_dir("/home/member/.fonts");\n'),
    ("timezone", "iana_time_zone",
     "let zone = iana_time_zone::get_timezone();\n"),
    ("timezone", "current_local_offset",
     "let offset = time::UtcOffset::current_local_offset();\n"),
    ("timezone", "TZ",
     'let zone = std::env::var("TZ");\n'),
    ("timezone", "TZ",
     'let zone = std::env::var_os(r"TZ");\n'),
    ("timezone", "TZ",
     'let zone = unsafe { libc::getenv(c"TZ".as_ptr()) };\n'),
    ("timezone", "TZ",
     'let zone = unsafe { libc::getenv(b"TZ\\0".as_ptr().cast()) };\n'),
    ("timezone", "now_local",
     "let now = time::OffsetDateTime::now_local()?;\n"),
    ("timezone", "chrono::Local",
     "use chrono::{DateTime, Local, Utc};\n"),
    ("timezone", "chrono::Local",
     "let shown: DateTime<Local> = stamp.into();\n"),
    ("timezone", "chrono::Local",
     "let shown = stamp.with_timezone(&Local);\n"),
    ("timezone", "chrono::Local",
     "let shown = Local.timestamp_opt(secs, 0);\n"),
    ("timezone", "localtime",
     "let parts = unsafe { libc::localtime(&t) };\n"),
    ("timezone", "tzname",
     "let name = unsafe { libc::tzname[0] };\n"),
    ("timezone", "TimeZone::system",
     "let zone = jiff::tz::TimeZone::system();\n"),
    ("timezone", "TimeZone::system",
     "let zone = jiff::tz::TimeZone::try_system()?;\n"),
    ("timezone", "TimeZoneSettings",
     "let name = windows::System::TimeZoneSettings::CurrentTimeZoneDisplayName()?;\n"),
    ("timezone", "GetTimeZone",
     "let zone = windows::Globalization::Calendar::new()?.GetTimeZone()?;\n"),
    ("timezone", "Zoned::now",
     "let now = jiff::Zoned::now();\n"),
    ("timezone", "libc::timezone",
     "let west = unsafe { libc::timezone };\n"),
    ("total memory", "GlobalMemoryStatus",
     "GlobalMemoryStatusEx(&mut status);\n"),
    ("total memory", "/proc/meminfo",
     'let memory = read_to_string("/proc/meminfo");\n'),
    ("processor model or count", "cpuid",
     "let leaf = unsafe { core::arch::x86_64::__cpuid(0x8000_0002) };\n"),
    ("processor model or count", "available_parallelism",
     "let cores = std::thread::available_parallelism();\n"),
    ("processor model or count", "brand_string",
     'let model = sysctl("machdep.cpu.brand_string");\n'),
    ("processor model or count", "HW_NCPU",
     "let mib = [libc::CTL_HW, libc::HW_NCPU];\n"),
    ("processor model or count", "HW_NCPU",
     "let mib = [libc::CTL_HW, libc::HW_AVAILCPU];\n"),
    ("processor model or count", "PROCESSOR_IDENTIFIER",
     'let model = std::env::var("PROCESSOR_IDENTIFIER");\n'),
    ("processor model or count", "NUMBER_OF_PROCESSORS",
     'let cores = std::env::var("NUMBER_OF_PROCESSORS");\n'),
    ("processor model or count", "PROCESSOR_REVISION",
     'let stepping = std::env::var_os("PROCESSOR_REVISION");\n'),
    ("processor model or count", "PROCESSOR_LEVEL",
     'let family = std::env::var_os("PROCESSOR_LEVEL");\n'),
    ("high-resolution timing correlator", "rdtsc",
     "let ticks = unsafe { core::arch::x86_64::_rdtsc() };\n"),
    ("high-resolution timing correlator", "QueryPerformanceFrequency",
     "QueryPerformanceFrequency(&mut frequency);\n"),
    ("high-resolution timing correlator", "mach_absolute_time",
     "let now = unsafe { mach_absolute_time() };\n"),
    ("high-resolution timing correlator", "CLOCK_MONOTONIC",
     "unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };\n"),
    ("high-resolution timing correlator", "CLOCK_MONOTONIC",
     "let ts = clock_gettime(ClockId::CLOCK_MONOTONIC_COARSE)?;\n"),
    ("high-resolution timing correlator", "/proc/stat",
     'let stat = read_to_string("/proc/stat")?;\n'),
    ("high-resolution timing correlator", "/proc/stat",
     'let boot = stat.lines().find(|l| l.starts_with("btime"));\n'),
    ("high-resolution timing correlator", "timeGetTime",
     "let since_boot = unsafe { windows::Win32::Media::timeGetTime() };\n"),
    ("high-resolution timing correlator", "CLOCK_UPTIME_RAW",
     "let up = unsafe { clock_gettime(CLOCK_UPTIME_RAW, &mut ts) };\n"),
    ("high-resolution timing correlator", "clock_gettime_nsec_np",
     "let up = unsafe { clock_gettime_nsec_np(8) };\n"),
    ("high-resolution timing correlator", "QueryInterruptTime",
     "unsafe { QueryUnbiasedInterruptTime(&mut ticks) };\n"),
    ("high-resolution timing correlator", "systemUptime",
     "let up = NSProcessInfo::processInfo().systemUptime();\n"),
    ("processor model or count", "processorCount",
     "let cores = unsafe { GetActiveProcessorCount(ALL_PROCESSOR_GROUPS) };\n"),
    ("processor model or count", "processorCount",
     "let cores = NSProcessInfo::processInfo().activeProcessorCount();\n"),
)

for category, name, body in CAUGHT:
    problems = tree(with_rust(body))[0]
    report(f"a read of {name} fails as a {category} source",
           mentions(problems, "crates/x/src/probe.rs:1", category, repr(name)))

problems = tree(with_rust('let id = read("/ETC/MACHINE-ID");\nlet g = "machineguid";\n'))[0]
report("a source spelled in another case is the same source",
       mentions(problems, "probe.rs:1", "'machine-id'")
       and mentions(problems, "probe.rs:2", "'MachineGuid'"))

problems = tree(with_rust(
    'let id = read("/etc/machine-\\\n        id");\n'
    'let g = "Machine\\\r\n    Gu\\\n  id";\n'
    'let w = "C:\\\\\nMachine";\n'
    "let n = gethostname();\n"
))[0]
report("a name a line continuation splits is the same name, on the line it starts",
       mentions(problems, "probe.rs:1", "'machine-id'")
       and mentions(problems, "probe.rs:3", "'MachineGuid'"))
report("...an escaped backslash ending a line continues nothing",
       not mentions(problems, "probe.rs:6", "'MachineGuid'")
       and not mentions(problems, "probe.rs:7", "'MachineGuid'"))
report("...and the lines after keep their numbers",
       mentions(problems, "probe.rs:8", "'gethostname'"))

problems = tree(with_rust('let key = r"SOFTWARE\\Microsoft\\Cryptography";\n'))[0]
report("the registry key that holds MachineGuid fails through a raw string",
       mentions(problems, "probe.rs:1", "Cryptography"))

problems = tree(with_rust(
    "use chrono::prelude::*;\n"
    "fn a() { let d = DateTime::<Local>::from(std::time::SystemTime::now()); }\n"
    "fn b() { let tz = Local; let _ = Utc::now().with_timezone(&tz); }\n"
    'fn c() -> &str { "Local State" }\n'
))[0]
report("chrono's Local through a turbofish is a read",
       mentions(problems, "probe.rs:2", "'chrono::Local'"))
report("...and, under chrono's prelude glob, a bare Local value is too",
       mentions(problems, "probe.rs:3", "'chrono::Local'"))
report("...while the word Local inside a literal is not, even there",
       not mentions(problems, "probe.rs:4"))

problems = tree(with_rust(
    "mod local { pub fn now() -> u32 { 0 } }\nfn a() -> u32 { local::now() + LOCAL::today() }\n"
))[0]
report("a module named local in another case is not chrono's Local", problems == [])

started = time.monotonic()
check.sources_in("chrono::{ Local, " * 1500)
report("an unclosed chrono import list cannot stall the check",
       time.monotonic() - started < 5)

problems = tree(with_rust(
    'let path = user_data.join("Local State");\nout.push_str("# Local Bookmark Store");\n'
))[0]
report("the word Local in ordinary prose is not chrono's Local", problems == [])

problems = tree(with_rust('let hostname = url.host_str();\nlet computername = 1;\n'))[0]
report("a variable named for a host name is not the environment variable", problems == [])

problems = tree({**with_rust('let zone = "tz";\nlet z = TZ_OFFSET;\n'),
                  "web/zone.js": "const zone = process.env.TZ;\n"})[0]
report("a TZ literal in another case, or a name that holds TZ, is not the variable",
       not mentions(problems, "probe.rs"))
report("...while a script reading env.TZ is",
       mentions(problems, "web/zone.js:1", "'TZ'"))

problems = tree({**with_rust(
    'let a = std::env::var("ComputerName");\n'
    'let b = std::env::var_os("processor_identifier");\n'
    'let c = ExpandEnvironmentStringsW(w!("%Number_Of_Processors%"));\n'
    'let d = std::env::var("hostname");\n'
), "web/probe.js": "const host = process.env.userDomain;\n"})[0]
report("a Windows environment variable named in another case is still a read",
       mentions(problems, "probe.rs:1", "'COMPUTERNAME'")
       and mentions(problems, "probe.rs:2", "'PROCESSOR_IDENTIFIER'")
       and mentions(problems, "probe.rs:3", "'NUMBER_OF_PROCESSORS'")
       and mentions(problems, "web/probe.js:1", "'USERDOMAIN'"))
report("...while POSIX, which reads its variables in their own case, is not",
       not mentions(problems, "probe.rs:4"))

problems = tree(with_rust(
    "let w = self.screen.width;\nlet t = self.performance.now;\nlet f = doc.document.fonts;\n"
))[0]
report("a Rust field access that spells a script path is not a read", problems == [])

problems = tree(with_rust(
    'const PROBE: &str = "send(screen.width, performance.now())";\n'
))[0]
report("...while the same path in a script the shell injects is",
       mentions(problems, "probe.rs:1", "'screen.'")
       and mentions(problems, "probe.rs:1", "'performance.now'"))

problems = tree(with_rust(
    'let id = read("/etc/machine\\x2did");\n'
    'let mem = read("/proc/mem\\u{69}nfo");\n'
    'const PROBE: &str = "performance\\x2enow()";\n'
))[0]
report("a source name spelled with a literal's escapes is the same name",
       mentions(problems, "probe.rs:1", "'machine-id'")
       and mentions(problems, "probe.rs:2", "'/proc/meminfo'")
       and mentions(problems, "probe.rs:3", "'performance.now'"))

problems = tree(with_rust(
    'let a = "/etc/machine\\\\x2did";\nlet b = "line\\x0a";\nlet guid = MachineGuid();\n'
))[0]
report("...while an escaped backslash is left as written, with what follows it",
       check.decode_escapes('"a\\\\x2d" "\\\\\\\\u{69}"') == '"a\\\\x2d" "\\\\\\\\u{69}"'
       and check.decode_escapes('"a\\\\\\x2d"') == '"a\\\\-"')
report("...and so is an escaped line break, so a later read keeps its line",
       mentions(problems, "probe.rs:3", "'MachineGuid'"))

problems = tree(with_rust(
    'let cpu = read(Path::new("/proc").join("cpuinfo"));\n'
    'let mem = read(Path::new("/proc").join("meminfo"));\n'
    'let up = read(Path::new("/proc").join("uptime"));\n'
))[0]
report("a /proc file named by joining its name onto /proc is the same read",
       mentions(problems, "probe.rs:1", "'/proc/cpuinfo'")
       and mentions(problems, "probe.rs:2", "'/proc/meminfo'")
       and mentions(problems, "probe.rs:3", "'/proc/uptime'"))

problems = tree(with_rust("let uptime = started.elapsed();\n"))[0]
report("...while a field named uptime is not", problems == [])

problems = tree(with_rust('let up = read(Path::new("/proc").join(r"uptime"));\n'))[0]
report("a source opening with punctuation is found after a word character too",
       mentions(problems, "probe.rs:1", "'/proc/uptime'"))

problems = tree(with_rust('const PROBE: &str = "let s=screen;";\n'))[0]
report("...and so is an alternative that opens with punctuation mid-pattern",
       mentions(problems, "probe.rs:1", "'window.screen'"))

problems, read = tree(passing_tree({
    "crates/x/src/probe.rs": (
        '#[path = "probe_impl.txt"]\nmod probe_impl;\n'
        'include!("generated.in");\n'
        'const PROBE: &str = include_str!("probe.js.txt");\n'
        '// include!("commented.in");\n'
    ),
    "crates/x/src/probe_impl.txt": 'let g = "MachineGuid";\n',
    "crates/x/src/generated.in": "let t = mach_absolute_time();\n",
    "crates/x/src/probe.js.txt": "send(screen.width);\n",
    "crates/x/src/commented.in": "let t = mach_absolute_time();\n",
}))[:2]
report("a file compiled in through #[path] is read as Rust whatever its suffix",
       mentions(problems, "crates/x/src/probe_impl.txt:1", "'MachineGuid'"))
report("...and so is one compiled in through include!",
       mentions(problems, "crates/x/src/generated.in:1", "'mach_absolute_time'"))
report("...and one embedded through include_str! is read whole",
       mentions(problems, "crates/x/src/probe.js.txt:1", "'screen.'"))
report("...while one named only in a comment is not read",
       "crates/x/src/commented.in" not in read)

problems = tree(passing_tree({
    "crates/x/src/probe.rs": (
        'include!["generated.in"];\nconst PROBE: &str = include_str!{"probe.js.txt"};\n'
    ),
    "crates/x/src/generated.in": "let t = mach_absolute_time();\n",
    "crates/x/src/probe.js.txt": "send(screen.width);\n",
}))[0]
report("a macro called with brackets or braces brings its file in too",
       mentions(problems, "crates/x/src/generated.in:1", "'mach_absolute_time'")
       and mentions(problems, "crates/x/src/probe.js.txt:1", "'screen.'"))

problems = tree(passing_tree({
    "crates/x/src/probe.rs": (
        'const PROBE: &[u8] = include_bytes!("inject.txt");\n'
    ),
    "crates/x/src/inject.txt": b"\xff navigator.hardwareConcurrency\n",
}))[0]
report("a file embedded through include_bytes! is read whole, bad UTF-8 replaced",
       mentions(problems, "crates/x/src/inject.txt:1", "'hardwareConcurrency'"))

problems = tree(passing_tree({
    "crates/x/src/probe.rs": (
        '#[cfg_attr(windows, path = "probe_windows.txt")]\n'
        '#[cfg_attr(not(windows), path = "probe_other.txt")]\nmod probe_impl;\n'
        '#[cfg_attr(feature = "path", derive(Debug))]\nstruct S;\n'
    ),
    "crates/x/src/probe_windows.txt": 'let g = "MachineGuid";\n',
    "crates/x/src/probe_other.txt": 'let id = read("/etc/machine-id");\n',
}))[0]
report("a file compiled in through cfg_attr's path is read, whichever cfg holds",
       mentions(problems, "crates/x/src/probe_windows.txt:1", "'MachineGuid'")
       and mentions(problems, "crates/x/src/probe_other.txt:1", "'machine-id'"))

problems = tree(passing_tree({
    "crates/x/src/lib.rs": (
        'mod outer {\n    fn f() {}\n    #[path = "inner.txt"]\n    mod inner;\n}\n'
        '#[path = "flat.txt"]\nmod flat;\n'
    ),
    "crates/x/src/probe.rs": 'pub mod nested {\n    #[path = "deep.txt"]\n    mod deep;\n}\n',
    "crates/x/src/outer/inner.txt": 'let g = "MachineGuid";\n',
    "crates/x/src/flat.txt": "let n = gethostname();\n",
    "crates/x/src/probe/nested/deep.txt": "let t = mach_absolute_time();\n",
}))[0]
report("a #[path] inside an inline module is read from that module's directory",
       mentions(problems, "crates/x/src/outer/inner.txt:1", "'MachineGuid'")
       and mentions(problems, "crates/x/src/flat.txt:1", "'gethostname'"))
report("...under the file's own name when that file is not mod.rs or a crate root",
       mentions(problems, "crates/x/src/probe/nested/deep.txt:1", "'mach_absolute_time'"))

for label, body in (
    ("a name too long for the system", 'include!("' + "a" * 300 + '.in");\n'),
    ("a NUL", 'const S: &str = include_str!("a\0b");\n'),
):
    try:
        problems = tree(with_rust(body))[0]
        report(f"a brought file with {label} is reported, not a traceback",
               mentions(problems, "probe.rs:", "cannot be resolved"))
    except (OSError, RuntimeError, ValueError) as error:
        report(f"a brought file with {label} is reported, not {error!r}", False)

with tempfile.TemporaryDirectory() as tmp:
    base = Path(tmp)
    root = base / "tree"
    for relative, content in with_rust('include!("loop.in");\n').items():
        (root / relative).parent.mkdir(parents=True, exist_ok=True)
        (root / relative).write_text(content, encoding="utf-8")
    (root / "crates" / "x" / "src" / "loop.in").symlink_to("loop.in")
    (base / "allowlist.txt").write_text("", encoding="utf-8")
    try:
        problems = check.check_tree(root, base / "allowlist.txt")[0]
        report("a brought file whose link loops is reported, not a traceback",
               mentions(problems, "probe.rs:1", "loop.in"))
    except (OSError, RuntimeError, ValueError) as error:
        report(f"a brought file whose link loops is reported, not {error!r}", False)

problems, read = tree(passing_tree({
    "crates/x/src/probe.rs": 'pub const S: &str = include_str!("payload.rs");\n',
    "crates/x/src/payload.rs": "const t = performance.now();\n",
}))[:2]
report("a Rust file embedded as text is read whole too, not only as Rust",
       mentions(problems, "crates/x/src/payload.rs:1", "'performance.now'"))
report("...and listed once among the files read",
       read.count("crates/x/src/payload.rs") == 1)

problems = tree(passing_tree({
    "crates/x/Cargo.toml": (
        CLEAN_MANIFEST.replace("[dependencies]", 'build = "build.txt"\n\n[dependencies]', 1)
        + '\n[lib]\npath = "src/lib.txt"\n\n[[bin]]\nname = "tool"\npath = "src/tool.txt"\n'
    ),
    "crates/x/build.txt": "fn main() { let n = gethostname(); }\n",
    "crates/x/src/lib.txt": 'pub fn g() { let g = "MachineGuid"; }\n',
    "crates/x/src/tool.txt": "fn main() { let t = mach_absolute_time(); }\n",
}))[0]
report("a crate root or build script a manifest names is read, whatever its suffix",
       mentions(problems, "crates/x/build.txt:1", "'gethostname'")
       and mentions(problems, "crates/x/src/lib.txt:1", "'MachineGuid'")
       and mentions(problems, "crates/x/src/tool.txt:1", "'mach_absolute_time'"))

problems = tree(with_rust('fn f() {}\ninclude!("missing.in");\n#[path = "gone.txt"]\nmod gone;\n'))[0]
report("a brought file that is not there is reported, not skipped",
       mentions(problems, "probe.rs:2", "missing.in", "no file")
       and mentions(problems, "probe.rs:3", "gone.txt", "no file"))

problems = tree(with_rust('fn f() {}\ninclude!("../../../../outside.in");\n'))[0]
report("a file brought in from outside the tree is reported, on the line that brings it",
       mentions(problems, "probe.rs:2", "outside.in", "outside the tree"))

problems = tree(with_rust(
    'const FIXTURE: &str = r#"include!("../../../../outside.in");"#;\n'
    'const QUOTED: &str = "#[path = \\"../../../../outside.rs\\"] mod m;";\n'
))[0]
report("a literal that quotes an include! or #[path] brings nothing in", problems == [])

problems = tree(with_rust("let a = xMachineGuid;\nlet b = MachineGuidx;\nlet c = MachineGuid_2;\n"))[0]
report("a source continued by an identifier on either side alone is not a read",
       problems == [])

problems = tree(with_rust("let a = (MachineGuid);\nlet b = [MachineGuid];\n"))[0]
report("...while one set off by punctuation on both sides is",
       mentions(problems, "probe.rs:1", "'MachineGuid'")
       and mentions(problems, "probe.rs:2", "'MachineGuid'"))

problems = tree(with_rust("let a = 1;\nlet b = 2;\nlet guid = MachineGuid();\n"))[0]
report("a failure names the line the read is on", mentions(problems, "probe.rs:3"))

problems = tree(passing_tree({"crates/x/notes.md": "MachineGuid, /etc/machine-id\n",
                              "crates/x/tool.py": "GetVolumeInformationW()\n"}))[0]
report("markdown and Python are not read", problems == [])

# --- the rotating salt: the derivation is caught, not its lifetime -----------

ROTATING_SALT = (
    "/// A per-install key, re-derived every day under a fresh salt, so no\n"
    "/// value it produces outlives the day it was made.\n"
    "pub fn daily_install_key(day: u32) -> [u8; 32] {\n"
    '    let guid = registry.get_string("MachineGuid");\n'
    "    let salt = day.to_be_bytes();\n"
    "    sha256(&[guid.as_bytes(), &salt].concat())\n"
    "}\n"
)
problems = tree(with_rust(ROTATING_SALT))[0]
report("a value re-derived under a daily-rotated salt is caught by the same rule",
       mentions(problems, "probe.rs:4", "'MachineGuid'"))

problems = tree(with_rust(
    "fn bucket(salt: u64) -> u64 { hash(salt ^ mach_absolute_time()) % 100 }\n"
))[0]
report("a rollout bucket drawn from a timing counter and a salt fails",
       mentions(problems, "high-resolution timing correlator", "'mach_absolute_time'"))

# --- SOURCE: script and markup ------------------------------------------------

problems, read = tree(passing_tree({
    "crates/x/ui/chrome.mjs": "const started = performance.now();\n",
}))[:2]
report("performance.now in shipped script fails",
       mentions(problems, "crates/x/ui/chrome.mjs:1", "'performance.now'"))
report("...and the script is read", "crates/x/ui/chrome.mjs" in read)

problems = tree(passing_tree({
    "crates/x/ui/index.html": "<script>send(screen.width, navigator.hardwareConcurrency)</script>\n",
}))[0]
report("screen geometry in markup fails",
       mentions(problems, "index.html:1", "screen geometry"))
report("...and so does the processor count on the same line",
       mentions(problems, "index.html:1", "'hardwareConcurrency'"))

problems = tree(passing_tree({
    "crates/x/ui/zone.TS": "const zone = Intl.DateTimeFormat().resolvedOptions().timeZone;\n",
}))[0]
report("a script suffix in another case is still read",
       mentions(problems, "zone.TS:1", "timezone"))

problems = tree(passing_tree({
    "crates/x/ui/zone.js": (
        "const zone = Intl.DateTimeFormat()\n"
        "  .resolvedOptions()\n"
        "  .timeZone;\n"
        "const { width, height } = window.screen;\n"
        "const depth = screen\n  .colorDepth;\n"
        "const tall = screen?.availHeight;\n"
        "const wide = screen['availWidth'];\n"
        "const { colorDepth } = screen;\n"
    ),
}))[0]
report("a chain a formatter breaks across lines is one read, on the line it starts",
       mentions(problems, "zone.js:2", "'resolvedOptions'"))
report("...and the screen object taken whole by destructuring is a read",
       mentions(problems, "zone.js:4", "'window.screen'")
       and mentions(problems, "zone.js:9", "'window.screen'"))
report("...as are a property split from screen, optionally chained or bracketed",
       mentions(problems, "zone.js:5", "'screen.'")
       and mentions(problems, "zone.js:7", "'screen.'")
       and mentions(problems, "zone.js:8", "'screen.'"))

problems = tree(passing_tree({
    "crates/x/ui/whole.js": (
        "const { width, height } = screen\n"
        "send(width)\r\n"
        "const { colorDepth } = screen\r\n"
        "const a = self.screen;\n"
        "const b = globalThis.screen;\n"
        "const c = window['screen'];\n"
    ),
}))[0]
report("the screen object taken whole is a read without a semicolon, LF or CRLF",
       mentions(problems, "whole.js:1", "'window.screen'")
       and mentions(problems, "whole.js:3", "'window.screen'"))
report("...and through self, globalThis or a bracketed window",
       mentions(problems, "whole.js:4", "'window.screen'")
       and mentions(problems, "whole.js:5", "'window.screen'")
       and mentions(problems, "whole.js:6", "'window.screen'"))

problems = tree(passing_tree({
    "crates/x/ui/brackets.js": (
        "for (const k in screen) out[k] = screen[k];\n"
        "const w = screen?.['width'];\n"
        "const t = performance['now']();\n"
        "const o = performance?.[`timeOrigin`];\n"
        "const c = navigator['connection'];\n"
        "const f = document[\"fonts\"];\n"
    ),
}))[0]
report("a bracketed read of a script object is a read, whatever the key's quotes",
       mentions(problems, "brackets.js:1", "'screen.'")
       and mentions(problems, "brackets.js:2", "'screen.'")
       and mentions(problems, "brackets.js:3", "'performance.now'")
       and mentions(problems, "brackets.js:4", "'performance.timeOrigin'")
       and mentions(problems, "brackets.js:5", "'navigator.connection'")
       and mentions(problems, "brackets.js:6", "'document.fonts'"))

problems = tree(passing_tree({
    "crates/x/ui/destructure.js": (
        "const { now } = performance;\n"
        "const { timeOrigin } = window.performance\n"
        "const { connection } = navigator;\n"
        "const { fonts } = document;\n"
        "const { language } = navigator;\n"
    ),
}))[0]
report("a property destructured out of a script object is a read",
       mentions(problems, "destructure.js:1", "'performance.now'")
       and mentions(problems, "destructure.js:2", "'performance.timeOrigin'")
       and mentions(problems, "destructure.js:3", "'navigator.connection'")
       and mentions(problems, "destructure.js:4", "'document.fonts'"))
report("...while destructuring a property no source names is not",
       not mentions(problems, "destructure.js:5"))

problems = tree(passing_tree({
    "crates/x/ui/escaped.js": (
        "const a = navigator['hardware\\x43oncurrency'];\n"
        "const b = navigator.hardware\\u0043oncurrency;\n"
        "const c = navigator['device\\u{4d}emory'];\n"
    ),
}))[0]
report("a source name spelled with script's escapes is the same name",
       mentions(problems, "escaped.js:1", "'hardwareConcurrency'")
       and mentions(problems, "escaped.js:2", "'hardwareConcurrency'")
       and mentions(problems, "escaped.js:3", "'deviceMemory'"))

problems = tree(passing_tree({
    "crates/x/ui/notes.js": "// performance.now would be a correlator here\n",
}))[0]
report("a source named in a script comment fails, the loud direction by design",
       mentions(problems, "notes.js:1", "'performance.now'"))

# --- the other script suffixes ------------------------------------------------

for suffix in (".htm", ".cjs", ".mts", ".cts"):
    problems = tree(passing_tree({f"crates/x/ui/probe{suffix}": "performance.now();\n"}))[0]
    report(f"a {suffix} file is read as script",
           mentions(problems, f"probe{suffix}:1", "'performance.now'"))

# --- DEPENDENCY ---------------------------------------------------------------


def with_manifest(tail):
    return passing_tree({"crates/x/Cargo.toml": CLEAN_MANIFEST + tail})


problems = tree(with_manifest('sysinfo = "0.30"\n'))[0]
report("a direct dependency on sysinfo fails",
       mentions(problems, "crates/x/Cargo.toml", "total memory", "'sysinfo'"))

problems = tree(with_manifest('system = { package = "machine-uid", version = "0.5" }\n'))[0]
report("a renamed dependency is read under its real name",
       mentions(problems, "Cargo.toml", "'machine-uid'"))

problems = tree(with_manifest("iana_time_zone = \"0.1\"\n"))[0]
report("a dependency name compares with - and _ folded",
       mentions(problems, "Cargo.toml", "timezone", "'iana-time-zone'"))

problems = tree(passing_tree({"crates/x/Cargo.toml": CLEAN_MANIFEST + (
    "\n[target.'cfg(windows)'.dependencies]\nwmi = \"0.13\"\n"
    "\n[dev-dependencies]\nfont-kit = \"0.14\"\n"
    "\n[build-dependencies]\nnum_cpus = \"1\"\n"
)}))[0]
report("a target-specific dependency fails", mentions(problems, "'wmi'"))
report("a dev-dependency fails", mentions(problems, "'font-kit'"))
report("a build-dependency fails", mentions(problems, "'num_cpus'"))

problems = tree(passing_tree({
    "Cargo.toml": '[workspace]\nmembers = []\n\n[workspace.dependencies]\nmac_address = "1"\n',
}))[0]
report("a workspace dependency fails", mentions(problems, "Cargo.toml", "'mac_address'"))

problems = tree(passing_tree({
    "Cargo.toml": (
        '[workspace]\nmembers = ["crates/x"]\n\n'
        '[workspace.dependencies]\nsys = { package = "sysinfo", version = "0.30" }\n'
    ),
    "crates/x/Cargo.toml": CLEAN_MANIFEST + "sys = { workspace = true }\n",
}))[0]
report("a member inheriting a renamed workspace dependency is read under its real name",
       mentions(problems, "crates/x/Cargo.toml", "'sysinfo'"))

problems = tree(passing_tree({
    "crates/y/cargo.toml": '[package]\nname = "y"\n\n[dependencies]\nsysinfo = "0.30"\n',
}))[0]
report("a manifest named in another case is read",
       mentions(problems, "crates/y/cargo.toml", "'sysinfo'"))

problems = tree(with_manifest(
    '\n[dev_dependencies]\nfont-kit = "0.14"\n\n[build_dependencies]\nnum_cpus = "1"\n'
))[0]
report("the underscore spellings of the dev and build tables are read",
       mentions(problems, "'font-kit'") and mentions(problems, "'num_cpus'"))

problems = tree(with_manifest('SysInfo = "0.30"\n'))[0]
report("a crate name compares with case folded",
       mentions(problems, "crates/x/Cargo.toml", "'sysinfo'"))

problems = tree(passing_tree({"crates/x/Cargo.toml": "[package\nname = \n"}))[0]
report("a manifest that is not TOML fails rather than passing unread",
       mentions(problems, "Cargo.toml", "not TOML"))

try:
    problems = tree(passing_tree({
        "crates/x/Cargo.toml": CLEAN_MANIFEST + 'sysinfo = "0.30"\n\n[package.metadata]\n',
        "crates/y/Cargo.toml": '[package]\nname = "y"\ntarget = "x"\n',
        "crates/z/Cargo.toml": 'target = ["x"]\n',
    }))[0]
    report("a target key that is not a table ends in a verdict, not a traceback",
           mentions(problems, "crates/x/Cargo.toml", "'sysinfo'"))
except AttributeError:
    report("a target key that is not a table ends in a verdict, not a traceback", False)

# --- ALLOWLIST ----------------------------------------------------------------

problems, _, allowlisted = tree(
    with_rust('let zone = std::env::var("TZ");\n'),
    allowlist="# the history view shows local time\ncrates/x/src/probe.rs TZ\n",
)
report("an allowlisted use passes", problems == [])
report("...and is counted", allowlisted == 1)

problems = tree(
    with_rust('let zone = std::env::var("TZ");\nlet g = "MachineGuid";\n'),
    allowlist="crates/x/src/probe.rs TZ\n",
)[0]
report("an entry permits its source only, not every source in the file",
       mentions(problems, "probe.rs:2", "'MachineGuid'")
       and not mentions(problems, "'TZ'"))

problems = tree(
    passing_tree({"crates/x/src/other.rs": 'let zone = std::env::var("TZ");\n'}),
    allowlist="crates/x/src/probe.rs TZ\n",
)[0]
report("an entry permits its file only",
       mentions(problems, "other.rs:1", "'TZ'"))
report("...and an entry permitting nothing fails as stale",
       mentions(problems, "fingerprinting-allowlist.txt:1", "permits nothing"))

problems = tree(with_manifest('sysinfo = "0.30"\n'),
                allowlist="crates/x/Cargo.toml sysinfo\n")[0]
report("a dependency is allowlisted by manifest path and crate name", problems == [])

problems = tree(
    passing_tree({"crates/x/src/with space.rs": "let t = GetTickCount64();\n"}),
    allowlist="crates/x/src/with space.rs GetTickCount\n",
)[0]
report("an entry's path may hold a space", problems == [])

problems = tree(passing_tree(), allowlist="crates/x/src/lib.rs NotASource\n")[0]
report("an entry naming an unknown source fails",
       mentions(problems, "fingerprinting-allowlist.txt:1", "names no source"))

problems = tree(passing_tree(), allowlist="MachineGuid\n")[0]
report("an entry that is not `<path> <source>` fails",
       mentions(problems, "fingerprinting-allowlist.txt:1", "<path> <source>"))

problems = tree(
    with_rust("let t = GetTickCount64();\n"),
    allowlist="crates/x/src/probe.rs GetTickCount\ncrates/x/src/probe.rs GetTickCount\n",
)[0]
report("an entry listed twice fails",
       mentions(problems, "fingerprinting-allowlist.txt:2", "listed twice"))

problems = tree(passing_tree(), allowlist=MISSING)[0]
report("a missing allowlist fails; a missing file is not an empty one",
       mentions(problems, "missing"))

problems = tree(passing_tree(), allowlist="# comments only\n\n   \n")[0]
report("an allowlist of comments and blank lines is empty and passes", problems == [])

# --- unreadable input and an unreached verdict -------------------------------

problems = tree(passing_tree({"crates/x/ui/bad.js": b"\xff\xfe performance.now()"}))[0]
report("script that is not UTF-8 fails rather than passing unread",
       mentions(problems, "bad.js", "not valid UTF-8"))

problems = tree(passing_tree({"crates/x/Cargo.toml": b"\xff\xfe[package]"}))[0]
report("a manifest that is not UTF-8 fails rather than passing unread",
       mentions(problems, "crates/x/Cargo.toml", "not valid UTF-8"))

problems = tree(passing_tree(), allowlist=b"\xff\xfe crates/x/src/lib.rs TZ\n")[0]
report("an allowlist that is not UTF-8 fails rather than passing unread",
       mentions(problems, "fingerprinting-allowlist.txt", "not valid UTF-8"))

problems = tree(passing_tree({"crates/x/src/bad.rs": b"\xff\xfe MachineGuid"}))[0]
report("Rust that is not UTF-8 fails rather than passing unread",
       mentions(problems, "bad.rs", "not valid UTF-8"))

# A byte-order mark is stripped where it would otherwise change the verdict:
# before an allowlist's first entry, where it would glue itself to the path,
# and before a manifest, which tomllib refuses with one.
problems = tree(
    with_rust('let zone = std::env::var("TZ");\n'),
    allowlist="\ufeffcrates/x/src/probe.rs TZ\n",
)[0]
report("a byte-order mark before the allowlist's first entry is not part of its path",
       problems == [])

problems = tree(passing_tree({
    "crates/x/Cargo.toml": "\ufeff" + CLEAN_MANIFEST + 'sysinfo = "0.30"\n',
}))[0]
report("...and one before a manifest is not a way past the dependency clause",
       mentions(problems, "crates/x/Cargo.toml", "'sysinfo'"))

problems = tree(passing_tree({
    "crates/x/target/debug/build/dep.rs": 'let g = "MachineGuid";\n',
    ".git/dep.rs": 'let g = "MachineGuid";\n',
    ".GIT/dep.rs": 'let g = "MachineGuid";\n',
}))[0]
report("outside a git work tree, .git/ in any case is not read, and target/ is",
       mentions(problems, "crates/x/target/debug/build/dep.rs:1")
       and not mentions(problems, ".git/")
       and not mentions(problems, ".GIT/"))

with tempfile.TemporaryDirectory() as tmp:
    base = Path(tmp)
    root = base / "tree"
    for relative, content in passing_tree({
        ".gitignore": "/target\n",
        "target/debug/build/dep.rs": 'let g = "MachineGuid";\n',
        "target/evil/src/main.rs": 'let id = read("/etc/machine-id");\n',
        "crates/x/src/Cargo.toml": "",
        "crates/x/src/target/mod.rs": "let n = gethostname();\n",
    }).items():
        (root / relative).parent.mkdir(parents=True, exist_ok=True)
        (root / relative).write_text(content, encoding="utf-8")
    (base / "allowlist.txt").write_text("", encoding="utf-8")
    subprocess.run(["git", "init", "-q", str(root)], check=True)
    subprocess.run(["git", "-C", str(root), "add", "-f", "target/evil/src/main.rs"], check=True)
    problems = check.check_tree(root, base / "allowlist.txt")[0]
    report("in a git work tree, build output git ignores is not read",
           not mentions(problems, "target/debug/"))
    report("...while a file committed under target/ is",
           mentions(problems, "target/evil/src/main.rs:1", "'machine-id'"))
    report("...and a stray Cargo.toml hides no module named target",
           mentions(problems, "crates/x/src/target/mod.rs:1", "'gethostname'"))

problems = tree(passing_tree({".probe/src/lib.rs": 'let g = "MachineGuid";\n'}))[0]
report("any other dot-directory is read, since a workspace member may live there",
       mentions(problems, ".probe/src/lib.rs:1", "'MachineGuid'"))

problems = tree(passing_tree({
    "crates/x/src/target/mod.rs": 'let g = "MachineGuid";\n',
    "crates/target/src/lib.rs": 'let g = "MachineGuid";\n',
}))[0]
report("a module named target is read, since Cargo builds it",
       mentions(problems, "crates/x/src/target/mod.rs:1", "'MachineGuid'"))
report("...and so is a crate named target",
       mentions(problems, "crates/target/src/lib.rs:1", "'MachineGuid'"))

with tempfile.TemporaryDirectory() as tmp:
    base = Path(tmp)
    root = base / "tree"
    for relative, content in passing_tree().items():
        (root / relative).parent.mkdir(parents=True, exist_ok=True)
        (root / relative).write_text(content, encoding="utf-8")
    (base / "outside").mkdir()
    (base / "outside" / "lib.rs").write_text('let g = "MachineGuid";\n', encoding="utf-8")
    (root / "crates" / "x" / "loop").symlink_to(root / "crates")
    (root / "crates" / "x" / "out").symlink_to(base / "outside")
    (root / "crates" / "x" / "src" / "linked.rs").symlink_to(base / "outside" / "lib.rs")
    (root / "crates" / "x" / "src" / "refused.rs").write_text("", encoding="utf-8")
    (root / "crates" / "x" / "sealed").mkdir()
    (base / "allowlist.txt").write_text("", encoding="utf-8")
    read_text = Path.read_text

    def refusing(path, *args, **kwargs):
        if path.name == "refused.rs":
            raise PermissionError(13, "Permission denied")
        return read_text(path, *args, **kwargs)

    iterdir = Path.iterdir

    def refusing_list(path):
        if path.name == "sealed":
            raise PermissionError(13, "Permission denied")
        return iterdir(path)

    Path.read_text, Path.iterdir = refusing, refusing_list
    try:
        problems, read, _ = check.check_tree(root, base / "allowlist.txt")
        report("a directory link that loops ends the walk rather than extending it",
               read.count("crates/x/src/lib.rs") == 1
               and not any("/loop/" in where for where in read))
        report("a directory the system refuses to list is reported, not a traceback",
               mentions(problems, "crates/x/sealed", "not a directory this check can list"))
        report("a directory link that leads outside the tree is reported, not read",
               mentions(problems, "crates/x/out", "outside the tree")
               and not mentions(problems, "'MachineGuid'"))
        report("...and so is a file link",
               mentions(problems, "crates/x/src/linked.rs", "outside the tree")
               and "crates/x/src/linked.rs" not in read)
        report("a file the system refuses to read is reported, not a traceback",
               mentions(problems, "crates/x/src/refused.rs", "not readable"))
    except OSError as error:
        report(f"links and unreadable files end in a verdict, not {error!r}", False)
    finally:
        Path.read_text, Path.iterdir = read_text, iterdir

try:
    tree({"README.md": "nothing this check reads\n"},
         allowlist="crates/evreos-shell/src/history_time.rs TZ\n")
    report("a tree with nothing to read raises even when every entry is stale in it",
           False)
except check.CheckError:
    report("a tree with nothing to read raises even when every entry is stale in it",
           True)

try:
    tree({"README.md": "nothing this check reads\n"})
    report("a tree with nothing to read raises rather than passing", False)
except check.CheckError:
    report("a tree with nothing to read raises rather than passing", True)

try:
    check.check_tree(Path(tempfile.gettempdir()) / "no-such-tree-for-this-check")
    report("a root that is not a directory raises", False)
except check.CheckError:
    report("a root that is not a directory raises", True)

# --- the command line ---------------------------------------------------------

result = run_main()
report("the default invocation passes on the repository", result.returncode == 0)
report("...ending in one line saying so",
       result.stdout.strip().startswith("Fingerprinting check passed:")
       and len(result.stdout.strip().splitlines()) == 1)

with tempfile.TemporaryDirectory() as tmp:
    root = Path(tmp)
    (root / "src").mkdir()
    (root / "src" / "lib.rs").write_text('let g = "MachineGuid";\n', encoding="utf-8")
    result = run_main("--root", str(root))
    report("a breach exits 1", result.returncode == 1)
    report("...naming the file and line on standard error",
           "src/lib.rs:1" in result.stderr and "MachineGuid" in result.stderr)

    empty = root / "empty"
    empty.mkdir()
    result = run_main("--root", str(empty))
    report("an unreached verdict exits 2", result.returncode == 2)

    (root / "src" / "lib.rs").write_text('let zone = std::env::var("TZ");\n', encoding="utf-8")
    listed = root / "listed.txt"
    listed.write_text("src/lib.rs TZ\n", encoding="utf-8")
    result = run_main("--root", str(root), "--allowlist", str(listed))
    report("--allowlist names the list the run reads", result.returncode == 0
           and "1 allowlisted uses" in result.stdout)

print(f"{PASSED}/{PASSED + FAILED} passed")
sys.exit(1 if FAILED else 0)
