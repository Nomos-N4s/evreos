"""The synthetic profile behind the import's 20,000-bookmark timings: a
Chromium `Bookmarks` file of 400 folders of 50 bookmarks each on the bar,
written into <directory>/Default so `import_timing` reads it as a profile."""
import json, os, sys

directory = os.path.join(sys.argv[1], 'Default')
os.makedirs(directory, exist_ok=True)


def bookmark(n):
    return {'type': 'url', 'name': f'Bookmark {n}', 'url': f'https://big.example/{n}',
            'date_added': '13344473600000000'}


folders = [{'type': 'folder', 'name': f'Folder {f}', 'children': [bookmark(f * 50 + i) for i in range(50)]}
           for f in range(400)]
roots = {'bookmark_bar': {'type': 'folder', 'name': 'bar', 'children': folders},
         'other': {'type': 'folder', 'children': []},
         'synced': {'type': 'folder', 'children': []}}
with open(os.path.join(directory, 'Bookmarks'), 'w') as out:
    json.dump({'roots': roots, 'version': 1}, out)
