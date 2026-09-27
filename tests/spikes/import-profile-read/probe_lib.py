"""Probe a live profile store through the SQLite library, the way an importer
built on rusqlite or any SQLite binding would read it."""
import shutil, sqlite3, sys, tempfile, os, struct

def header(path):
    with open(path, 'rb') as f:
        h = f.read(100)
    return {
        'page_size': struct.unpack('>H', h[16:18])[0],
        'write_version': h[18], 'read_version': h[19],  # 1 legacy/rollback, 2 WAL
        'change_counter': struct.unpack('>I', h[24:28])[0],
        'pages_in_header': struct.unpack('>I', h[28:32])[0],
    }

def journal_state(path):
    j = path + '-journal'
    if not os.path.exists(j):
        return 'absent'
    with open(j, 'rb') as f:
        b = f.read(8)
    if len(b) == 0:
        return 'empty'
    return 'HOT (magic present)' if b == bytes.fromhex('d9d505f920a163d7') else f'size {os.path.getsize(j)}, header zeroed'

def direct(path, query):
    try:
        c = sqlite3.connect(f'file:{path}?mode=ro', uri=True, timeout=0.5)
        n = c.execute(query).fetchone()[0]
        c.close()
        return f'ok, {n} rows'
    except sqlite3.Error as e:
        return f'FAILED: {e}'

def copy_read(path, query, with_sidecars):
    d = tempfile.mkdtemp()
    dst = os.path.join(d, os.path.basename(path))
    shutil.copyfile(path, dst)
    if with_sidecars:
        for s in ('-wal', '-journal'):
            if os.path.exists(path + s):
                shutil.copyfile(path + s, dst + s)
    try:
        c = sqlite3.connect(f'file:{dst}?mode=ro', uri=True)
        n = c.execute(query).fetchone()[0]
        c.close()
        return f'ok, {n} rows'
    except sqlite3.Error as e:
        return f'FAILED: {e}'
    finally:
        shutil.rmtree(d)

path, query = sys.argv[1], sys.argv[2]
print(path)
print('  header:', header(path))
print('  journal:', journal_state(path), '| wal:', os.path.getsize(path + '-wal') if os.path.exists(path + '-wal') else 'absent',
      '| shm:', 'present' if os.path.exists(path + '-shm') else 'absent')
print('  direct read (library, mode=ro):', direct(path, query))
print('  copy main only, then read:     ', copy_read(path, query, False))
print('  copy main + sidecars, then read:', copy_read(path, query, True))
