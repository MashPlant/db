#!/usr/bin/python
import csv

out = 'a.csv'
ans = 'tests/sql/a.csv'

out = csv.reader(open(out))
ans = csv.reader(open(ans))

out = '\n'.join(map(lambda x: ', '.join(x), out)).strip()
ans = '\n'.join(map(lambda x: ', '.join(x), ans)).strip()

print(out == ans)