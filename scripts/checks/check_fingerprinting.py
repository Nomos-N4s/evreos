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

It reads the tree and fails on:

  SOURCE        a read of one of the characteristics below, in Rust source,
                with comments stripped and string literals kept through
                rustlex: the paths and registry names these reads use live
                inside strings, so blanking strings would blank the evidence,
                and a script the shell injects is a string too.
                Every source is matched as a whole token with case folded,
                because registry names and paths are case-insensitive on the
                release platforms and a spelling nobody used is still the same
                key.

    machine and volume identifiers
                the Windows MachineGuid and the Cryptography key that holds
                it, `/etc/machine-id` and the D-Bus copy of it, the macOS
                platform UUID and serial number, the SMBIOS and DMI tables
                and the serial fields read from them, the WMI hardware
                classes, the WinRT hardware and system identifiers and the
                advertising identifier, the device model, the host name, and
                volume serial numbers and UUIDs.
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
                script.
    installed fonts
                enumerating the system's font collection, on every platform
                API, through fontconfig and `fc-list`, through a font
                library's system-font loader, and by listing a system font
                directory.
    timezone
                the system timezone and the local UTC offset, through every
                platform API, `/etc/localtime`, the `TZ` variable, the
                `time` and `chrono` crates' local-time entry points, and
                `getTimezoneOffset` and `resolvedOptions().timeZone` in
                script.
    total memory
                the physical memory the machine carries, through every
                platform API, `/proc/meminfo`, `sysinfo` and
                `navigator.deviceMemory`.
    processor model and count
                the processor's brand string, CPUID, the registry and sysctl
                keys that name the processor, `/proc/cpuinfo`, and the
                processor count -- `available_parallelism`, `num_cpus`,
                `hardwareConcurrency` and their platform equivalents. The
                task names the model; the count is read here too because
                FR-036a binds on the characteristic, not on the one field a
                crash reporter happens to label it with, and the count is the
                same kind of fact about the same part.
    high-resolution timing correlators
                the raw counters whose origin or rate is a fact about the
                machine rather than an interval the shell measured: `rdtsc`,
                `QueryPerformanceCounter` and `QueryPerformanceFrequency`,
                `mach_absolute_time` and its continuous twin,
                `CLOCK_MONOTONIC_RAW`, `CLOCK_BOOTTIME`, `GetTickCount`, the
                boot time and uptime, and `performance.now` and
                `performance.timeOrigin` in script -- the class research
                section 4.3 names.

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

A derivation spread across files, or behind a wrapper whose name says nothing,
rests on review. And none of this touches what a SITE does to fingerprint the
member, which research section 4.3 sets out of FR-036a's scope and out of this
architecture's reach.

Files other than Rust source are not read: Python is the tooling that runs this
check and ships in nothing, and markdown is where the forbidden sources are
quoted.
Directories are matched with case folded where they must be, the release
platforms' filesystems folding case. Dot-directories and `target/` are not
read: nothing under either ships.
"""
import argparse
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent

sys.path.insert(0, str(HERE))
from casefs import folded_in, is_rust_source  # noqa: E402
from rustlex import strip_non_code  # noqa: E402

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
        ("navigator.connection", r"navigator\.connection"),
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
        ("screen.", r"screen\.(?:width|height|availWidth|availHeight|availLeft|availTop|colorDepth|pixelDepth|orientation)"),
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
        ("document.fonts", r"document\.fonts"),
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
        ("localtime_r", r"localtime_[rs]"),
        ("tm_gmtoff", r"tm_gmtoff"),
        ("tzset", r"tzset"),
        ("current_local_offset", r"current_local_offset"),
        ("local_offset_at", r"local_offset_at"),
        ("chrono::Local", r"chrono::Local|Local::(?:now|today)"),
        ("TZ", r"var(?:_os)?\(\s*\"TZ\"\s*\)"),
        ("getTimezoneOffset", r"getTimezoneOffset"),
        ("resolvedOptions().timeZone", r"resolvedOptions\(\s*\)\.timeZone"),
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
        ("processorCount", r"(?:active)?processorCount"),
    ),
    "high-resolution timing correlator": (
        ("rdtsc", r"_*rdtscp?"),
        ("QueryPerformanceCounter", r"QueryPerformanceCounter"),
        ("QueryPerformanceFrequency", r"QueryPerformanceFrequency"),
        ("mach_absolute_time", r"mach_absolute_time"),
        ("mach_continuous_time", r"mach_continuous_time"),
        ("CLOCK_MONOTONIC_RAW", r"CLOCK_MONOTONIC_RAW"),
        ("CLOCK_BOOTTIME", r"CLOCK_BOOTTIME"),
        ("GetTickCount", r"GetTickCount(?:64)?"),
        ("/proc/uptime", r"/proc/uptime"),
        ("kern.boottime", r"kern\.boottime"),
        ("KERN_BOOTTIME", r"KERN_BOOTTIME"),
        ("performance.now", r"performance\.now"),
        ("performance.timeOrigin", r"performance\.timeOrigin"),
    ),
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


def sources_in(line):
    """Every (category, name) of a source read on `line`, in table order,
    each once."""
    found = []
    for category, name, pattern in COMPILED:
        if pattern.search(line) and (category, name) not in found:
            found.append((category, name))
    return found


def check_tree(root):
    """Every clause over the tree at `root`.

    Returns (problems, files read), the second a sorted list of the
    repository-relative POSIX paths
    this check read. An empty `problems` is a pass -- unless nothing was
    read, which raises
    CheckError instead: a check over nothing is not a pass, and it is not a
    breach of FR-036a either, so it must not exit 1 as one. A root that is not
    a directory raises the same.
    """
    root = Path(root).resolve()
    if not root.is_dir():
        raise CheckError(f"{root}: not a directory this check can read")

    problems = []
    read = []

    def found(where, number, category, name):
        at = f"{where}:{number}" if number else where
        problems.append(
            f"{at}: reads {category} source {name!r}; FR-036a forbids deriving "
            "an identifier or correlator from it, however the value is hashed, "
            "salted or kept"
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
                for number, line in enumerate(code.splitlines(), 1):
                    for category, name in sources_in(line):
                        found(where, number, category, name)

    if not problems and not read:
        raise CheckError(
            f"{root}: no Rust source; a check over nothing is not a pass"
        )
    return problems, sorted(read)


def walk(root):
    """Every directory under `root` this check reads, `root` included.

    Dot-directories and `target/` are pruned: nothing under either ships,
    and `target/` holds every vendored dependency's source, which is not
    this tree's to fix. The fold on `target` is casefs's rule -- `TARGET/`
    is the same directory on the platforms that build the release.
    """
    kept = [root]
    for directory in kept:
        for path in sorted(directory.iterdir()):
            if not path.is_dir():
                continue
            if path.name.startswith(".") or folded_in(path.name, ["target"]):
                continue
            kept.append(path)
    return kept


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--root", default=str(REPO), help="tree to check; the repository by default"
    )
    args = parser.parse_args()

    try:
        problems, read = check_tree(args.root)
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

    print(f"Fingerprinting check passed: {len(read)} files read.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
