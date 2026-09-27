"""Naive single-pass copies of the synthetic store, checked by the SQLite
library: a copy is torn if quick_check fails or it holds two generations."""
import os, shutil, sqlite3, sys, tempfile, time
mode, directory, seconds = sys.argv[1], sys.argv[2], float(sys.argv[3])
name, table, col = (('History', 'urls', 'last_visit_time') if mode == 'rollback'
                    else ('places.sqlite', 'moz_places', 'last_visit_date'))
src = os.path.join(directory, name)
while not os.path.exists(src): time.sleep(0.1)
time.sleep(1)
trials = torn = unreadable = 0
work = tempfile.mkdtemp()
end = time.time() + seconds
while time.time() < end:
    trials += 1
    dst = os.path.join(work, f'c{trials}')
    shutil.copyfile(src, dst)
    if os.path.exists(src + '-wal'):
        shutil.copyfile(src + '-wal', dst + '-wal')
    try:
        c = sqlite3.connect(dst)
        ok = c.execute('pragma quick_check').fetchone()[0] == 'ok'
        gens = c.execute(f'select count(distinct {col}) from {table}').fetchone()[0]
        c.close()
        torn += (not ok) or gens != 1
    except sqlite3.Error:
        unreadable += 1
    for s in ('', '-wal', '-journal', '-shm'):
        if os.path.exists(dst + s): os.remove(dst + s)
shutil.rmtree(work)
print(f'naive copy, {mode}: trials={trials} torn={torn} unreadable={unreadable}')
