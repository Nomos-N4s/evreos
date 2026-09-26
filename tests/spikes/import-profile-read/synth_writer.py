"""A worst-case writer: every transaction rewrites every row, stamping one
generation number into all of them, so a consistent copy holds exactly one
generation and a torn copy holds two. Locking and journal modes copy the
browsers': exclusive locking, rollback journal (Chromium) or WAL (Firefox)."""
import os, random, sqlite3, sys, time

mode, directory, seconds = sys.argv[1], sys.argv[2], float(sys.argv[3])
os.makedirs(directory, exist_ok=True)
if mode == 'rollback':
    path, table, url_col, time_col = os.path.join(directory, 'History'), 'urls', 'url', 'last_visit_time'
    schema = ['CREATE TABLE urls(id INTEGER PRIMARY KEY AUTOINCREMENT,url LONGVARCHAR,title LONGVARCHAR,'
              'visit_count INTEGER DEFAULT 0 NOT NULL,typed_count INTEGER DEFAULT 0 NOT NULL,'
              'last_visit_time INTEGER NOT NULL,hidden INTEGER DEFAULT 0 NOT NULL)']
    base = 13_344_473_600_000_000
else:
    path, table, url_col, time_col = os.path.join(directory, 'places.sqlite'), 'moz_places', 'url', 'last_visit_date'
    schema = ['CREATE TABLE moz_places (id INTEGER PRIMARY KEY, url LONGVARCHAR, title LONGVARCHAR, '
              'visit_count INTEGER DEFAULT 0, hidden INTEGER DEFAULT 0 NOT NULL, last_visit_date INTEGER)',
              'CREATE TABLE moz_bookmarks (id INTEGER PRIMARY KEY, type INTEGER, fk INTEGER DEFAULT NULL, '
              'parent INTEGER, position INTEGER, title LONGVARCHAR, dateAdded INTEGER, guid TEXT)']
    base = 1_700_000_000_000_000
db = sqlite3.connect(path, isolation_level=None)
db.execute('PRAGMA page_size=4096')
db.execute('PRAGMA locking_mode=EXCLUSIVE')
if mode == 'wal':
    db.execute('PRAGMA journal_mode=WAL')
    db.execute('PRAGMA wal_autocheckpoint=64')
for sql in schema:
    db.execute(sql)
db.execute('BEGIN')
for n in range(3000):
    db.execute(f'INSERT INTO {table}({url_col},title,{time_col}) VALUES (?,?,?)',
               (f'https://gen.example/{n}', 't', base))
db.execute('COMMIT')
gen, end = 0, time.time() + seconds
while time.time() < end:
    gen += 1
    db.execute('BEGIN')
    db.execute(f'UPDATE {table} SET {time_col}=?, title=?', (base + gen * 1000, 'x' * random.randint(5, 400)))
    db.execute('COMMIT')
print(f'{mode}: {gen} transactions, each rewriting all 3000 rows')
