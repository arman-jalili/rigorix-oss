import re, sys

adr = sys.argv[1]
# mods, declared, items, deps, narrow (narrowed tokio), direct, closure, crates
mods, declared, items, deps, narrow, direct, closure, crates = (int(v) for v in sys.argv[2:10])

NOUN = {
    'public modules': ('the module count', (mods, declared)),
    'modules': ('the module count', (mods, declared)),
    'inner items': ('the item count', (items,)),
    'items': ('the item count', (items,)),
    'dependencies': ('the dependency count', (deps,)),
    'crates': ('the crate count', (crates,)),
    # "16 direct" is the two buckets together; the forbidden-crate bucket alone is 10.
    'direct': ('the direct-exclusion count (both buckets)', (direct + narrow,)),
    'closure': ('the closure count', (closure,)),
}

# Pronouns and elliptical forms: `Of those 22`, `only 10 leave for a forbidden crate`,
# `6 leave because they need a tokio feature`, `6 because they name a symbol`. Each names
# its subject by construction, so each gets a subject — and anything left unmatched below
# is a failure rather than a skip, which is the whole point of the rule.
IDIOM = [
    (re.compile(r'Of those (\d+)'), 'the module count', (mods,)),
    (re.compile(r'only (\d+)\s+leave for a forbidden crate'), 'the forbidden-crate count', (direct,)),
    (re.compile(r'(\d+)\s+(?:leave )?because they need'), 'the narrowed-tokio count', (narrow,)),
    (re.compile(r'(\d+)\s+(?:leave )?because they name'), 'the closure count', (closure,)),
]
# A count with a verb-shaped neighbour and no recognised subject.
SUSPECT = re.compile(r'(?<![\w.])(\d+)\s+(leave|because|more)\b')

NOUNS = '|'.join(sorted((re.escape(n) for n in NOUN), key=len, reverse=True))
NOUN_RE = re.compile(r'(?<![\w.])(\d{1,3}(?:[  ]\d{3})*|\d+)\s+(' + NOUNS + r')\b')

failures = []
RATIO = re.compile(r'\d+\s+of\s+\d+')
LINES = open(adr, encoding='utf-8').readlines()

for lineno, raw in enumerate(LINES, start=1):
    line = raw.replace('**', '')
    # `12 of 22 leave …` is a ratio, and the ratio table already derives it. Blank the whole
    # expression (same length, so spans stay valid) before looking for bare counts, or the 22
    # would be read a second time as a count with a verb for a subject.
    line = RATIO.sub(lambda m: ' ' * len(m.group(0)), line)
    covered = []          # character spans already accounted for

    def judge(value, subject, allowed, text, span):
        covered.append(span)
        if value not in allowed:
            want = ' or '.join(str(a) for a in allowed)
            failures.append(f"{adr.rsplit('/', 1)[-1]}:{lineno} states '{text}' but {subject} is {want}")

    for m in NOUN_RE.finditer(line):
        value = int(m.group(1).replace(' ', ''))
        subject, allowed = NOUN[m.group(2)]
        judge(value, subject, allowed, m.group(0), m.span())

    # Idioms are matched against the line JOINED WITH THE NEXT ONE. Prose wraps, and
    # `**only 10\nleave for a forbidden crate**` is one claim: matched per line it read as
    # "10" followed by nothing, which is how a stale `11` passed this check once already.
    look = line if lineno == len(LINES) else line + ' ' + LINES[lineno].replace('**', '').rstrip('\n')
    for pattern, subject, allowed in IDIOM:
        for m in pattern.finditer(look):
            if m.start() >= len(line.rstrip('\n')):
                continue          # the claim lives on the next line; that line will judge it
            judge(int(m.group(1)), subject, allowed, m.group(0), m.span())

    for m in SUSPECT.finditer(line):
        if any(s <= m.start() < e for s, e in covered):
            continue
        failures.append(f"{adr.rsplit('/', 1)[-1]}:{lineno} states '{m.group(0)}' and the guard "
                        f"cannot derive a subject for it — cannot derive is a failure, not a skip")

for f in failures:
    print(f"  bare count: {f}")
sys.exit(1 if failures else 0)
