import csv

out = 'a.csv'
ans = 'tests/sql/a.csv'

out = csv.reader(open(out))
ans = csv.reader(open(ans))

out = '\n'.join(map(lambda x: ', '.join(x), out))
ans = '\n'.join(map(lambda x: ', '.join(x), ans))

print(out == ans)