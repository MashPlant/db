#!/usr/bin/python
import sys

for f in sys.argv[1:]:
    with open(f) as f:
        s = f.read()
        for s in s.splitlines():
            if s:
                if 'error' in s:
                    print(f'err!(e, "{s}");')
                else:
                    print(f'ok!(e, "{s}");')
            else:
                print()
