"""Copy a live store repeatedly, one naive pass per trial, and check each copy
with the SQLite library's own integrity check. Counts torn copies."""
import os, shutil, sqlite3, sys, tempfile, time

path, query, seconds = sys.argv[1], sys.argv[2], float(sys.argv[3])
sidecars = sys.argv[4] == 'sidecars'
MAGIC = bytes.fromhex('d9d505f920a163d7')
def hot():
    try:
        with open(path + '-journal', 'rb') as f:
            return f.read(8) == MAGIC
    except FileNotFoundError:
        return False
trials = torn = hot_seen = failed_open = 0
rows = []
end = time.time() + seconds
d = tempfile.mkdtemp()
while time.time() < end:
    dst = os.path.join(d, 'copy')
    for s in ('', '-wal', '-journal'):
        if os.path.exists(dst + s): os.remove(dst + s)
    h0 = hot()
    shutil.copyfile(path, dst)
    if sidecars and os.path.exists(path + '-wal'):
        shutil.copyfile(path + '-wal', dst + '-wal')
    h1 = hot()
    hot_seen += h0 or h1
    trials += 1
    try:
        c = sqlite3.connect(dst)
        ok = c.execute('pragma quick_check').fetchone()[0]
        n = c.execute(query).fetchone()[0]
        c.close()
        rows.append(n)
        if ok != 'ok':
            torn += 1
    except sqlite3.Error as e:
        failed_open += 1
    time.sleep(0.01)
shutil.rmtree(d)
print(f'{os.path.basename(path)} sidecars={sidecars}: trials={trials} torn(quick_check!=ok)={torn} '
      f'unreadable={failed_open} hot-journal-observed={hot_seen} rows first/last={rows[:1]}/{rows[-1:]}')
