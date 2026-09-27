#!/usr/bin/env python3
"""Enforce FR-036a's fingerprinting prohibition, over the whole tree.

WHAT THIS CHECKS, and why it reads for sources rather than for identifiers.

FR-036a: neither the shell nor any app it hosts may derive, store or transmit
any identifier or correlator for a device or a member from device, display,
font, network or timing characteristics, or from any combination of them,
however long it persists. "Stable" is not the test: a value re-derived from the
same characteristics under a salt rotated daily identifies the member all the
same, so the prohibition binds on the DERIVATION and not on the lifetime of what
it produces. Research section 4.3 reads that against what a browser does by
default -- the hardware-seeded install identifier an update client, a rollout
bucket or a crash grouper reaches for, and the device fields every crash
reporter ships -- and this check is the half of it a scanner can carry.

A derivation needs an input, and the input is where the scanner can see it. So
the rule is on the SOURCE: every read of a characteristic listed below fails
the build wherever it appears, whatever is done with the value afterwards.
Hashing it, salting it, rotating the salt, keeping it in memory only, or never
sending it changes nothing here, which is exactly FR-036a's point: the check
does not ask how long a value lives, so a rotating salt has nothing to argue
with. The update client, the staged-rollout draw and the shell therefore hold
no value derived from these sources while this check passes -- the draw is a
random number (`getrandom`), which is not a characteristic of the device and
is not read here.

A use that is not a derivation -- and there will be some: a history view that
shows local time needs the timezone -- is not waved through by a pattern. It is
listed, by file and by source, in scripts/checks/fingerprinting-allowlist.txt,
which is empty in v1 so that the first entry lands as a visible diff and the
pull request that writes it says why that read derives nothing. An entry is a
use taken, not one granted ahead of it: an entry that permits nothing the tree
does fails, so the list cannot outlive the code it excused.

It reads the tree and fails on:

  SOURCE        a read of one of the characteristics below, in Rust source,
                with comments stripped and string literals kept through
                rustlex: the paths and registry names these reads use live
                inside strings, so blanking strings would blank the evidence,
                and a script the shell injects is a string too. The
                script-shaped sources -- `screen.`, `window.screen`,
                `document.fonts`, `performance.now`, `performance.timeOrigin`
                and `navigator.connection` -- are dotted paths an ordinary
                Rust field access can spell, `self.screen.width` among them,
                so in Rust they are matched inside string literals only,
                where an injected script lives. A literal's `\\x` and
                `\\u{...}` escapes are decoded before matching, so a name
                spelled with an escape -- `"/etc/machine\\x2did"` -- is the
                same name.
                Script and markup the shell could ship -- `.js`, `.mjs`,
                `.cjs`, `.ts`, `.mts`, `.cts`, `.html`, `.htm` -- are read
                whole, comments included: there is no shared scanner for
                those languages, and a mention in a comment failing loudly is
                the safer direction than a read hidden behind a string that
                looks like a comment opener.
                Every source is matched as a whole token with case folded,
                because registry names and paths are case-insensitive on the
                release platforms and a spelling nobody used is still the same
                key.

    machine and volume identifiers
                the Windows MachineGuid and the Cryptography key that holds
                it, `/etc/machine-id` and the D-Bus copy of it, the host id
                (`gethostid`, `/etc/hostid`) and the per-boot `boot_id`, the
                macOS platform UUID and serial number, the SMBIOS and DMI
                tables and the serial fields read from them -- directly or
                through `dmidecode`, `wmic`, `ioreg` and `system_profiler`
                -- the WMI hardware classes, the WinRT hardware and system
                identifiers and the advertising identifier, the device
                model, also as script reads it through
                `getHighEntropyValues`, the host name -- through the
                platform APIs, POSIX `uname` and its `nodename`,
                `/proc/sys/kernel/hostname`, the macOS host-name calls, the
                Windows `COMPUTERNAME`, `USERDOMAIN` and `LOGONSERVER`
                variables and the POSIX `HOSTNAME` one -- and volume serial
                numbers and UUIDs.
    MAC addresses and network characteristics
                the adapter tables and ioctls that yield a MAC address or the
                machine's own interface addresses, `/sys/class/net`, Wi-Fi
                network identity (BSSID and the WLAN and CoreWLAN
                interfaces), the connection-type interfaces, and
                `RTCPeerConnection`, which is how script learns local
                addresses.
    screen geometry
                enumerating monitors or screens and reading their size,
                resolution, colour depth or arrangement, on every platform
                API the release tiers carry and on the `screen` object in
                script, whether a property is read off it -- dotted,
                optionally chained or bracketed -- or the object is taken
                whole, as destructuring takes it. Every dotted script
                source allows whitespace and a line break between its
                parts, as a formatter writes a long chain.
    installed fonts
                enumerating the system's font collection, on every platform
                API, through fontconfig and `fc-list`, through a font
                library's system-font loader, and by listing a system font
                directory.
    timezone
                the system timezone and the local UTC offset, through every
                platform API and libc's `localtime`, `tzname` and
                `timezone`, `/etc/localtime`, the `TZ` variable, the `time`
                crate's `now_local` and local offsets, chrono's `Local`
                wherever a line names it -- by path, in an import list, as
                `&Local`, `Local.` or `DateTime<Local>` -- jiff's system
                zone, and `getTimezoneOffset` and `resolvedOptions` in
                script. `Local` is matched in its own case only: the word
                opens ordinary prose such as "Local State".
    total memory
                the physical memory the machine carries, through every
                platform API, `/proc/meminfo`, `sysinfo` and
                `navigator.deviceMemory`.
    processor model and count
                the processor's brand string, CPUID, the registry and sysctl
                keys that name the processor, `/proc/cpuinfo`, and the
                processor count -- `available_parallelism`, `num_cpus`,
                `hardwareConcurrency` and their platform equivalents,
                `GetActiveProcessorCount` and its family among them -- and
                the Windows `PROCESSOR_IDENTIFIER`, `PROCESSOR_REVISION`,
                `PROCESSOR_LEVEL` and `NUMBER_OF_PROCESSORS` variables. The
                task names the model; the count is read here too because
                FR-036a binds on the characteristic, not on the one field a
                crash reporter happens to label it with, and the count is the
                same kind of fact about the same part.
    high-resolution timing correlators
                the raw counters whose origin or rate is a fact about the
                machine rather than an interval the shell measured: `rdtsc`,
                `QueryPerformanceCounter` and `QueryPerformanceFrequency`,
                `mach_absolute_time` and its continuous twin,
                `CLOCK_MONOTONIC_RAW`, `CLOCK_BOOTTIME`, `CLOCK_UPTIME_RAW`,
                `clock_gettime_nsec_np`, `GetTickCount`, the Windows
                interrupt-time counters, the boot time and uptime --
                `systemUptime` on macOS among them -- and `performance.now` and
                `performance.timeOrigin` in script -- the class research
                section 4.3 names.

  DEPENDENCY    a direct dependency, in any table of any `Cargo.toml` --
                ordinary, dev, build, target-specific or the workspace's own
                -- on a crate that exists to read those characteristics:
                `machine-uid`, `sysinfo`, `iana-time-zone`, `font-kit`,
                `mac_address` and the rest of DEPENDENCY_SOURCES below. A
                crate renamed with `package = ...` is read under its real
                name, and so is one a member inherits with `workspace =
                true` from a rename its workspace makes. Such a crate reads
                the source inside code this check never sees, so the
                manifest line is the one place the use is visible. Names
                compare with `-` and `_` folded, as crates.io folds them.

  ALLOWLIST     an entry in scripts/checks/fingerprinting-allowlist.txt that
                is not `<path> <source>`, names a source this check does not
                know, is listed twice, or permits nothing: no use of that
                source in that file. A missing allowlist is a failure too; a
                missing file is not an empty one.

WHAT THIS DOES NOT CATCH, stated so nothing is assumed of it.

A window's own scale factor -- winit's `scale_factor()`, script's
`devicePixelRatio` -- is not read. Every window reads it to render at the size
the member chose, which Principle X's scaling to 200% requires, and the
accessibility spike already does; a rule on it would fail every conformant
window, the chrome's among them. A derivation that uses it rests on review.

`std::time::Instant` is not read: it is opaque, and all it yields is an
interval between two readings in one process, which the tab model uses for its
load timeouts. A correlator built from such intervals -- timing a fixed
workload to fingerprint the processor -- is composed from innocent parts and
rests on review.

The locale is not a source here: FR-035 has the shell read the member's
language, and FR-039c's closed report contents and FR-039d's closed counter
keys already keep it out of crash reports and counters.

A crate that reads a characteristic and is reached only transitively is not
read: the lockfile holds crates other crates use for their own purposes, and
what this check answers for is what Evreos itself holds.

A derivation spread across files, or behind a wrapper whose name says nothing,
rests on review. And none of this touches what a SITE does to fingerprint the
member, which research section 4.3 sets out of FR-036a's scope and out of this
architecture's reach.

Files other than Rust source, the script and markup suffixes above and
`Cargo.toml` are not read: Python is the tooling that runs this check and ships
in nothing, and markdown is where the forbidden sources are quoted.
Directories are matched with case folded where they must be, the release
platforms' filesystems folding case. `.git/` is not read, and neither is
Cargo's build output: a `target/` directory beside a `Cargo.toml`, which
holds every vendored dependency's source and ships nothing of this tree's. A
directory named `target` anywhere else is read like any other, since a module
or a crate may carry that name and Cargo builds it. Every other directory whose
name starts with a dot is read: Cargo builds a workspace member wherever its
manifest names it.
"""
import argparse
import re
import sys
import tomllib
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent
ALLOWLIST = HERE / "fingerprinting-allowlist.txt"

sys.path.insert(0, str(HERE))
from casefs import folded_in, is_rust_source, suffix_of  # noqa: E402
from rustlex import strip_non_code  # noqa: E402

# Script and markup the shell could ship, read whole.
SCRIPT_SUFFIXES = (".js", ".mjs", ".cjs", ".ts", ".mts", ".cts", ".html", ".htm")

# The manifest this check reads dependencies from.
MANIFEST = "Cargo.toml"

# Every source, by category. A source is (name, pattern): the name is what a
# failure reports and what an allowlist entry spells, the pattern is matched as
# a whole token with case folded. A name holds no whitespace, because an
# allowlist entry is `<path> <name>` and a path may.
SOURCES = {
    "machine and volume identifier": (
        ("MachineGuid", r"MachineGuid"),
        ("Microsoft\\Cryptography", r"Microsoft[\\/]+Cryptography"),
        ("machine-id", r"machine-id"),
        ("machine_uid", r"machine_uid"),
        ("IOPlatformUUID", r"k?IOPlatformUUID(?:Key)?"),
        ("IOPlatformSerialNumber", r"k?IOPlatformSerialNumber(?:Key)?"),
        ("IOPlatformExpertDevice", r"IOPlatformExpertDevice"),
        ("gethostuuid", r"gethostuuid"),
        ("kern.uuid", r"kern\.uuid"),
        ("/sys/class/dmi", r"/sys/(?:class|devices/virtual)/dmi"),
        ("product_uuid", r"product_uuid"),
        ("product_serial", r"product_serial"),
        ("board_serial", r"board_serial"),
        ("chassis_serial", r"chassis_serial"),
        ("SMBIOS", r"SMBIOS"),
        ("GetSystemFirmwareTable", r"GetSystemFirmwareTable"),
        ("Win32_ComputerSystem", r"Win32_ComputerSystem(?:Product)?"),
        ("Win32_BIOS", r"Win32_BIOS"),
        ("Win32_BaseBoard", r"Win32_BaseBoard"),
        ("Win32_DiskDrive", r"Win32_DiskDrive"),
        ("Win32_PhysicalMedia", r"Win32_PhysicalMedia"),
        ("Win32_LogicalDisk", r"Win32_LogicalDisk"),
        ("Win32_Volume", r"Win32_Volume"),
        ("HardwareIdentification", r"HardwareIdentification"),
        ("GetPackageSpecificToken", r"GetPackageSpecificToken"),
        ("SystemIdentification", r"SystemIdentification"),
        ("GetSystemIdForPublisher", r"GetSystemIdForPublisher"),
        ("GetSystemIdForUser", r"GetSystemIdForUser"),
        ("EasClientDeviceInformation", r"EasClientDeviceInformation"),
        ("AdvertisingManager", r"AdvertisingManager"),
        ("AdvertisingId", r"AdvertisingId"),
        ("SystemProductName", r"SystemProductName"),
        ("SystemManufacturer", r"SystemManufacturer"),
        ("hw.model", r"hw\.model"),
        ("gethostname", r"gethostname"),
        ("GetComputerName", r"GetComputerName(?:Ex)?[AW]?"),
        ("hostname::get", r"hostname::get"),
        ("/etc/hostname", r"/etc/hostname"),
        ("/proc/sys/kernel/hostname", r"/proc/sys/kernel/hostname"),
        ("uname", r"uname"),
        ("utsname", r"utsname"),
        ("nodename", r"nodename"),
        ("gethostid", r"gethostid"),
        ("/etc/hostid", r"/etc/hostid"),
        ("boot_id", r"boot_id"),
        ("hostName", r"(?-i:hostName)"),
        ("SCDynamicStoreCopyComputerName", r"SCDynamicStoreCopyComputerName"),
        ("SCDynamicStoreCopyLocalHostName", r"SCDynamicStoreCopyLocalHostName"),
        ("NSHost", r"NSHost"),
        ("dmidecode", r"dmidecode"),
        ("wmic", r"wmic"),
        ("ioreg", r"ioreg"),
        ("system_profiler", r"system_profiler"),
        ("getHighEntropyValues", r"getHighEntropyValues"),
        # Environment variables, in the case Windows and POSIX spell them: a
        # host name is read as often from the environment as from an API.
        ("COMPUTERNAME", r"(?-i:COMPUTERNAME)"),
        ("USERDOMAIN", r"(?-i:USERDOMAIN)"),
        ("LOGONSERVER", r"(?-i:LOGONSERVER)"),
        ("HOSTNAME", r"(?-i:HOSTNAME)"),
        ("GetVolumeInformation", r"GetVolumeInformation(?:ByHandle)?[AW]?"),
        ("VolumeSerialNumber", r"VolumeSerialNumber"),
        ("/dev/disk/by-", r"/dev/disk/by-(?:uuid|id|partuuid|label)"),
        ("blkid", r"blkid"),
        ("DADiskCopyDescription", r"DADiskCopyDescription"),
        ("kDADiskDescriptionVolumeUUIDKey", r"kDADiskDescriptionVolumeUUIDKey"),
    ),
    "MAC address or network characteristic": (
        ("GetAdaptersAddresses", r"GetAdaptersAddresses"),
        ("GetAdaptersInfo", r"GetAdaptersInfo"),
        ("GetIfTable", r"GetIfTable2?"),
        ("SIOCGIFHWADDR", r"SIOCGIFHWADDR"),
        ("/sys/class/net", r"/sys/class/net"),
        ("getifaddrs", r"getifaddrs"),
        ("mac_address", r"mac_address"),
        ("MacAddress", r"MacAddress"),
        ("PhysicalAddress", r"PhysicalAddress"),
        ("GetHostNames", r"GetHostNames"),
        ("NetworkInformation", r"NetworkInformation"),
        ("navigator.connection", r"navigator\s*\??\.\s*connection"),
        ("RTCPeerConnection", r"RTCPeerConnection"),
        ("WlanQueryInterface", r"WlanQueryInterface"),
        ("WlanGetNetworkBssList", r"WlanGetNetworkBssList"),
        ("CWWiFiClient", r"CWWiFiClient"),
        ("bssid", r"bssid"),
    ),
    "screen geometry": (
        ("available_monitors", r"available_monitors"),
        ("primary_monitor", r"primary_monitor"),
        ("current_monitor", r"current_monitor"),
        ("MonitorHandle", r"MonitorHandle"),
        ("EnumDisplayMonitors", r"EnumDisplayMonitors"),
        ("GetMonitorInfo", r"GetMonitorInfo[AW]?"),
        ("EnumDisplayDevices", r"EnumDisplayDevices[AW]?"),
        ("EnumDisplaySettings", r"EnumDisplaySettings(?:Ex)?[AW]?"),
        ("GetSystemMetrics", r"GetSystemMetrics(?:ForDpi)?"),
        ("GetDeviceCaps", r"GetDeviceCaps"),
        ("NSScreen", r"NSScreen"),
        ("CGDisplay", r"CGDisplay(?:Bounds|PixelsWide|PixelsHigh|CopyDisplayMode|ScreenSize)"),
        ("CGMainDisplayID", r"CGMainDisplayID"),
        ("CGGetActiveDisplayList", r"CGGetActiveDisplayList"),
        ("gdk_screen", r"gdk_screen_\w+"),
        ("gdk_monitor", r"gdk_monitor_\w+"),
        ("gdk::Screen", r"gdk::Screen"),
        ("gdk::Monitor", r"gdk::Monitor"),
        ("XDisplayWidth", r"XDisplayWidth"),
        ("XDisplayHeight", r"XDisplayHeight"),
        ("XRRGetScreenResources", r"XRRGetScreenResources(?:Current)?"),
        ("screen.", (
            r"screen\s*(?:\??\.\s*(?:{0})|\[\s*[\"'](?:{0})[\"']\s*\])".format(
                "width|height|availWidth|availHeight|availLeft|availTop"
                "|colorDepth|pixelDepth|orientation"
            )
        )),
        # The screen object taken whole, as destructuring takes it:
        # `const { width } = window.screen`, `= screen;`.
        ("window.screen", r"window\s*\??\.\s*screen(?!\s*\??\.\s*\w)|=\s*screen(?=\s*[;,)])"),
        ("getScreenDetails", r"getScreenDetails"),
    ),
    "installed fonts": (
        ("EnumFontFamilies", r"EnumFontFamilies(?:Ex)?[AW]?"),
        ("EnumFonts", r"EnumFonts[AW]?"),
        ("GetSystemFontCollection", r"GetSystemFontCollection"),
        ("IDWriteFontCollection", r"IDWriteFontCollection\d?"),
        ("CTFontManagerCopyAvailable", r"CTFontManagerCopyAvailable\w+"),
        ("CTFontCollectionCreateFromAvailableFonts", r"CTFontCollectionCreateFromAvailableFonts"),
        ("NSFontManager", r"NSFontManager"),
        ("availableFonts", r"availableFonts"),
        ("availableFontFamilies", r"availableFontFamilies"),
        ("FcFontList", r"FcFontList"),
        ("FcConfigGetFonts", r"FcConfigGetFonts"),
        ("FcFontSetList", r"FcFontSetList"),
        ("fc-list", r"fc-list"),
        ("load_system_fonts", r"load_system_fonts"),
        ("font_kit", r"font_kit"),
        ("queryLocalFonts", r"queryLocalFonts"),
        ("document.fonts", r"document\s*\??\.\s*fonts"),
        ("/usr/share/fonts", r"/usr/share/fonts"),
        ("/Library/Fonts", r"/Library/Fonts"),
        ("Windows\\Fonts", r"Windows[\\/]+Fonts"),
    ),
    "timezone": (
        ("iana_time_zone", r"iana_time_zone"),
        ("get_timezone", r"get_timezone"),
        ("GetTimeZoneInformation", r"GetTimeZoneInformation(?:ForYear)?"),
        ("GetDynamicTimeZoneInformation", r"GetDynamicTimeZoneInformation"),
        ("/etc/localtime", r"/etc/localtime"),
        ("/etc/timezone", r"/etc/timezone"),
        ("NSTimeZone", r"NSTimeZone"),
        ("CFTimeZoneCopy", r"CFTimeZoneCopy(?:System|Default)"),
        ("localtime", r"localtime(?:_[rs])?"),
        ("tzname", r"tzname"),
        ("libc::timezone", r"libc::timezone"),
        ("tm_gmtoff", r"tm_gmtoff"),
        ("tzset", r"tzset"),
        ("current_local_offset", r"current_local_offset"),
        ("local_offset_at", r"local_offset_at"),
        ("now_local", r"now_local"),
        ("chrono::Local", (
            r"chrono::(?:offset::)?Local|Local::(?:now|today)"
            r"|chrono::(?:offset::|prelude::)?\{[^}]*(?-i:\bLocal\b)[^}]*\}"
            r"|DateTime\s*<\s*(?-i:Local)\s*>|&\s*(?-i:Local)|(?-i:Local)\s*\.\s*\w+"
        )),
        ("TimeZone::system", r"TimeZone::system"),
        ("Zoned::now", r"Zoned::now"),
        ("TZ", r'var(?:_os)?\(\s*(?:r#*)?"TZ"#*\s*\)'),
        ("getTimezoneOffset", r"getTimezoneOffset"),
        # The whole options object carries the zone, so the call is the read
        # whether `.timeZone` follows it or a destructuring takes it.
        ("resolvedOptions", r"resolvedOptions"),
    ),
    "total memory": (
        ("GlobalMemoryStatus", r"GlobalMemoryStatus(?:Ex)?"),
        ("GetPhysicallyInstalledSystemMemory", r"GetPhysicallyInstalledSystemMemory"),
        ("ullTotalPhys", r"ullTotalPhys"),
        ("_SC_PHYS_PAGES", r"_SC_PHYS_PAGES"),
        ("hw.memsize", r"hw\.memsize"),
        ("HW_MEMSIZE", r"HW_MEMSIZE"),
        ("HW_PHYSMEM", r"HW_PHYSMEM"),
        ("/proc/meminfo", r"/proc/meminfo"),
        ("sysinfo", r"sysinfo"),
        ("totalram", r"totalram"),
        ("total_memory", r"total_memory"),
        ("physicalMemory", r"physicalMemory"),
        ("deviceMemory", r"deviceMemory"),
    ),
    "processor model or count": (
        ("/proc/cpuinfo", r"/proc/cpuinfo"),
        ("cpuid", r"_*cpuid(?:_count)?"),
        ("raw_cpuid", r"raw_cpuid"),
        ("brand_string", r"brand_string"),
        ("machdep.cpu", r"machdep\.cpu(?:\.\w+)*"),
        ("ProcessorNameString", r"ProcessorNameString"),
        ("CentralProcessor", r"CentralProcessor"),
        ("Win32_Processor", r"Win32_Processor"),
        ("GetLogicalProcessorInformation", r"GetLogicalProcessorInformation(?:Ex)?"),
        ("GetSystemInfo", r"Get(?:Native)?SystemInfo"),
        ("dwNumberOfProcessors", r"dwNumberOfProcessors"),
        ("num_cpus", r"num_cpus"),
        ("available_parallelism", r"available_parallelism"),
        ("_SC_NPROCESSORS", r"_SC_NPROCESSORS_(?:ONLN|CONF)"),
        ("hw.ncpu", r"hw\.ncpu"),
        ("hw.physicalcpu", r"hw\.physicalcpu"),
        ("hw.logicalcpu", r"hw\.logicalcpu"),
        ("/sys/devices/system/cpu", r"/sys/devices/system/cpu"),
        ("hardwareConcurrency", r"hardwareConcurrency"),
        ("PROCESSOR_IDENTIFIER", r"(?-i:PROCESSOR_IDENTIFIER)"),
        ("PROCESSOR_REVISION", r"(?-i:PROCESSOR_REVISION)"),
        ("PROCESSOR_LEVEL", r"(?-i:PROCESSOR_LEVEL)"),
        ("NUMBER_OF_PROCESSORS", r"(?-i:NUMBER_OF_PROCESSORS)"),
        ("processorCount", r"(?:Get|KeQuery)?(?:Active|Maximum)?ProcessorCount"),
    ),
    "high-resolution timing correlator": (
        ("rdtsc", r"_*rdtscp?"),
        ("QueryPerformanceCounter", r"QueryPerformanceCounter"),
        ("QueryPerformanceFrequency", r"QueryPerformanceFrequency"),
        ("mach_absolute_time", r"mach_absolute_time"),
        ("mach_continuous_time", r"mach_continuous_time"),
        ("CLOCK_MONOTONIC_RAW", r"CLOCK_MONOTONIC_RAW"),
        ("CLOCK_BOOTTIME", r"CLOCK_BOOTTIME"),
        ("CLOCK_UPTIME_RAW", r"CLOCK_UPTIME_RAW"),
        ("clock_gettime_nsec_np", r"clock_gettime_nsec_np"),
        ("QueryInterruptTime", r"Query(?:Unbiased)?InterruptTime(?:Precise)?"),
        ("systemUptime", r"systemUptime"),
        ("GetTickCount", r"GetTickCount(?:64)?"),
        ("/proc/uptime", r"/proc/uptime"),
        ("kern.boottime", r"kern\.boottime"),
        ("KERN_BOOTTIME", r"KERN_BOOTTIME"),
        ("performance.now", r"performance\s*\??\.\s*now"),
        ("performance.timeOrigin", r"performance\s*\??\.\s*timeOrigin"),
    ),
}

# An escape inside a Rust literal that spells one character: `\x2d`,
# `\u{69}`. An escaped backslash is matched first, so `\\x2d` stays the four
# characters it is.
ESCAPE = re.compile(r"\\(\\|x[0-9A-Fa-f]{2}|u\{[0-9A-Fa-f_]{1,8}\})")

# The sources shaped as script's dotted paths, which a Rust field access can
# also spell: in Rust they are matched inside string literals only.
SCRIPT_SHAPED = {
    "screen.",
    "window.screen",
    "document.fonts",
    "performance.now",
    "performance.timeOrigin",
    "navigator.connection",
}

# A whole token: nothing that could continue an identifier on either side, so
# `sysinfo` is not found inside `mysysinfo_cache` and `bssid` not inside a
# longer word.
EDGE_BEFORE = r"(?<![A-Za-z0-9_])"
EDGE_AFTER = r"(?![A-Za-z0-9_])"

COMPILED = [
    (category, name, re.compile(EDGE_BEFORE + "(?:" + pattern + ")" + EDGE_AFTER, re.IGNORECASE))
    for category, sources in SOURCES.items()
    for name, pattern in sources
]

# Crates that exist to read one of the characteristics above, by the category
# they read. Keyed by the name crates.io publishes, compared with `-` and `_`
# folded.
DEPENDENCY_SOURCES = {
    "machine-uid": "machine and volume identifier",
    "machineid-rs": "machine and volume identifier",
    "wmi": "machine and volume identifier",
    "smbios-lib": "machine and volume identifier",
    "hostname": "machine and volume identifier",
    "gethostname": "machine and volume identifier",
    "whoami": "machine and volume identifier",
    "mac_address": "MAC address or network characteristic",
    "get_if_addrs": "MAC address or network characteristic",
    "if-addrs": "MAC address or network characteristic",
    "local-ip-address": "MAC address or network characteristic",
    "network-interface": "MAC address or network characteristic",
    "pnet_datalink": "MAC address or network characteristic",
    "display-info": "screen geometry",
    "font-kit": "installed fonts",
    "fontdb": "installed fonts",
    "iana-time-zone": "timezone",
    "sysinfo": "total memory",
    "sys-info": "total memory",
    "systemstat": "total memory",
    "heim": "total memory",
    "raw-cpuid": "processor model or count",
    "num_cpus": "processor model or count",
}

# Every table name Cargo reads dependencies from.
DEPENDENCY_TABLES = (
    "dependencies",
    "dev-dependencies",
    "dev_dependencies",
    "build-dependencies",
    "build_dependencies",
)


def fold_crate(name):
    """A crate name as crates.io compares it: case and `-`/`_` folded."""
    return name.lower().replace("_", "-")


FOLDED_DEPENDENCY_SOURCES = {fold_crate(name): name for name in DEPENDENCY_SOURCES}

# Every name an allowlist entry may spell: a source's, or a dependency's.
KNOWN_NAMES = {name for _, name, _ in COMPILED} | set(DEPENDENCY_SOURCES)


class CheckError(Exception):
    """The check could not reach a verdict: the tree it was pointed at is
    missing, or holds nothing it reads. main() reports this and exits 2 --
    the code scripts/checks/README.md reserves for an unreached verdict,
    which fails the workflow exactly as a breach does but reads differently
    in the log -- as the sibling checks exit for the same class."""


def read_text(path):
    """The file's text, or None when it is not UTF-8; the caller reports.

    The BOM is stripped for the reason the other checks strip it: an editor
    that writes one is not a way past a check.
    """
    try:
        return path.read_text(encoding="utf-8").lstrip("﻿")
    except UnicodeDecodeError:
        return None


def decode_escapes(text):
    """`text` with each `\\x` and `\\u{...}` escape replaced by the
    character it spells.

    An escape that spells a line break is left as written, and so is one that
    spells no character, so the line a read is reported on is the line it is
    written on.
    """
    def one(match):
        body = match.group(1)
        if body == "\\":
            return match.group(0)
        digits = body[1:].strip("{}").replace("_", "")
        try:
            character = chr(int(digits, 16))
        except (ValueError, OverflowError):
            return match.group(0)
        return match.group(0) if character in "\r\n" else character

    return ESCAPE.sub(one, text)


def sources_in(text, only=None, skip=()):
    """Every (line number, category, name) of a source read in `text`, in
    line order and then table order, each source once per line: of every
    source, or of those named in `only`, less those named in `skip`.

    Matched over the whole text rather than line by line, so a chain a
    formatter breaks across lines -- `screen` on one, `.width` on the next --
    is still one read, reported on the line it starts.
    """
    found = []
    for category, name, pattern in COMPILED:
        if (only is not None and name not in only) or name in skip:
            continue
        for match in pattern.finditer(text):
            number = text.count("\n", 0, match.start()) + 1
            if (number, category, name) not in found:
                found.append((number, category, name))
    return sorted(found, key=lambda item: item[0])


def dependencies_in(manifest, inherited=None):
    """Every crate a parsed manifest depends on directly, by its real name.

    Every dependency table Cargo reads: top-level, `target.<cfg>.`, and the
    workspace's own `[workspace.dependencies]`. A renamed dependency --
    `alias = { package = "sysinfo" }` -- is returned under the crate it
    names, since that is the code that runs. So is one a member inherits
    with `workspace = true`: `inherited` is its workspace's
    `[workspace.dependencies]`, where the rename is written, and the member
    cannot restate it.
    """
    inherited = inherited if isinstance(inherited, dict) else {}
    tables = []
    for key in DEPENDENCY_TABLES:
        tables.append(manifest.get(key))
    for target in (manifest.get("target") or {}).values():
        if isinstance(target, dict):
            for key in DEPENDENCY_TABLES:
                tables.append(target.get(key))
    workspace = manifest.get("workspace")
    if isinstance(workspace, dict):
        tables.append(workspace.get("dependencies"))

    names = []
    for table in tables:
        if not isinstance(table, dict):
            continue
        for alias, spec in table.items():
            real = alias
            if isinstance(spec, dict) and spec.get("workspace") is True:
                spec = inherited.get(alias)
            if isinstance(spec, dict) and isinstance(spec.get("package"), str):
                real = spec["package"]
            if real not in names:
                names.append(real)
    return names


def read_allowlist(path, problems):
    """The allowlist's entries, as {(path, source): line number}.

    One entry per line, `<path> <source>`: a repository-relative POSIX path
    and a source name as a failure reports it. `#` starts a comment and blank
    lines are ignored. The path is everything before the last run of
    whitespace, so a path holding a space is still one path.
    """
    where = path.name
    if not path.is_file():
        problems.append(
            f"{where}: missing; the allowlist is a committed file read on every "
            "run, and a missing file is not an empty one"
        )
        return {}
    text = read_text(path)
    if text is None:
        problems.append(f"{where}: not valid UTF-8, so its entries cannot be read")
        return {}
    entries = {}
    for number, line in enumerate(text.splitlines(), 1):
        entry = line.split("#", 1)[0].strip()
        if not entry:
            continue
        parts = entry.rsplit(None, 1)
        if len(parts) != 2:
            problems.append(
                f"{where}:{number}: {entry!r} is not `<path> <source>`; an entry "
                "names the file and the source it permits there"
            )
            continue
        key = (parts[0], parts[1])
        if key[1] not in KNOWN_NAMES:
            problems.append(
                f"{where}:{number}: {key[1]!r} names no source this check knows; "
                "copy the name from the failure the entry answers"
            )
            continue
        if key in entries:
            problems.append(f"{where}:{number}: {key[0]} {key[1]} is listed twice")
            continue
        entries[key] = number
    return entries


def check_tree(root, allowlist_path=ALLOWLIST):
    """Every clause over the tree at `root`.

    Returns (problems, files read, allowlisted uses): the second a sorted
    list of the repository-relative POSIX paths the SOURCE and DEPENDENCY
    clauses read, the third how many allowlist entries answered a use. An
    empty `problems` is a pass -- unless nothing was read, which raises
    CheckError instead: a check over nothing is not a pass, and it is not a
    breach of FR-036a either, so it must not exit 1 as one. A root that is not
    a directory raises the same.
    """
    root = Path(root).resolve()
    if not root.is_dir():
        raise CheckError(f"{root}: not a directory this check can read")

    problems = []
    allowlist_path = Path(allowlist_path)
    allowed = read_allowlist(allowlist_path, problems)
    used = set()
    read = []
    # Each workspace's [workspace.dependencies], by the directory of its
    # manifest. The walk reaches a workspace root before its members, which
    # sit beneath it.
    workspaces = {}

    def found(where, number, category, name):
        if (where, name) in allowed:
            used.add((where, name))
            return
        at = f"{where}:{number}" if number else where
        problems.append(
            f"{at}: reads {category} source {name!r}; FR-036a forbids deriving "
            "an identifier or correlator from it, however the value is hashed, "
            f"salted or kept, and {allowlist_path.name} lists no use of it here"
        )

    for directory in walk(root):
        for path in sorted(directory.iterdir()):
            if not path.is_file():
                continue
            where = path.relative_to(root).as_posix()
            if is_rust_source(path):
                text = read_text(path)
                read.append(where)
                if text is None:
                    problems.append(f"{where}: not valid UTF-8, so it is not Rust this check can read")
                    continue
                code = strip_non_code(text, keep_literals=True)
                bare = strip_non_code(text)
                literals = "".join(
                    kept if kept != blank else ("\n" if kept == "\n" else " ")
                    for kept, blank in zip(code, bare)
                )
                reads = sources_in(decode_escapes(code), skip=SCRIPT_SHAPED)
                reads += sources_in(decode_escapes(literals), only=SCRIPT_SHAPED)
                for number, category, name in sorted(reads, key=lambda item: item[0]):
                    found(where, number, category, name)
            elif suffix_of(path) in SCRIPT_SUFFIXES:
                text = read_text(path)
                read.append(where)
                if text is None:
                    problems.append(f"{where}: not valid UTF-8, so it is not script this check can read")
                    continue
                for number, category, name in sources_in(text):
                    found(where, number, category, name)
            elif folded_in(path.name, [MANIFEST]):
                text = read_text(path)
                read.append(where)
                if text is None:
                    problems.append(f"{where}: not valid UTF-8, so it is not a manifest this check can read")
                    continue
                try:
                    manifest = tomllib.loads(text)
                except tomllib.TOMLDecodeError as error:
                    problems.append(f"{where}: not TOML this check can read ({error})")
                    continue
                workspace = manifest.get("workspace")
                if isinstance(workspace, dict):
                    workspaces[path.parent] = workspace.get("dependencies")
                inherited = next(
                    (workspaces[d] for d in (path.parent, *path.parent.parents) if d in workspaces),
                    None,
                )
                for crate in dependencies_in(manifest, inherited):
                    name = FOLDED_DEPENDENCY_SOURCES.get(fold_crate(crate))
                    if name is not None:
                        found(where, 0, DEPENDENCY_SOURCES[name], name)

    for (where, name), number in sorted(allowed.items(), key=lambda item: item[1]):
        if (where, name) not in used:
            problems.append(
                f"{allowlist_path.name}:{number}: {where} {name} permits nothing; "
                "no use of that source is in that file, and an entry records a "
                "use taken, never one granted ahead of it"
            )

    if not problems and not read:
        raise CheckError(
            f"{root}: no Rust source, script or manifest; a check over nothing "
            "is not a pass"
        )
    return problems, sorted(read), len(used)


def walk(root):
    """Every directory under `root` this check reads, `root` included.

    `.git/` is pruned, and so is Cargo's build output: a `target/`
    beside a `Cargo.toml`, which holds every vendored dependency's source and
    is not this tree's to fix. Only there -- `src/target/` is a module and
    `crates/target/` a crate, and Cargo builds both. The fold on `target` and
    on the manifest name is casefs's rule -- `TARGET/` is the same directory
    on the platforms that build the release.
    """
    kept = [root]
    for directory in kept:
        entries = sorted(directory.iterdir())
        beside_manifest = any(
            path.is_file() and folded_in(path.name, [MANIFEST]) for path in entries
        )
        for path in entries:
            if not path.is_dir():
                continue
            if folded_in(path.name, [".git"]):
                continue
            if beside_manifest and folded_in(path.name, ["target"]):
                continue
            kept.append(path)
    return kept


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--root", default=str(REPO), help="tree to check; the repository by default"
    )
    parser.add_argument(
        "--allowlist", default=str(ALLOWLIST), help="the fingerprinting allowlist to read"
    )
    args = parser.parse_args()

    try:
        problems, read, allowlisted = check_tree(args.root, args.allowlist)
    except CheckError as error:
        print(f"Fingerprinting check could not run: {error}", file=sys.stderr)
        return 2

    if problems:
        print("Fingerprinting check FAILED:\n", file=sys.stderr)
        for problem in problems:
            print(f"  - {problem}", file=sys.stderr)
        print(
            "\nSee the docstring of scripts/checks/check_fingerprinting.py for "
            "the sources read and what satisfies each.",
            file=sys.stderr,
        )
        return 1

    print(
        f"Fingerprinting check passed: {len(read)} files read, "
        f"{allowlisted} allowlisted uses."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
