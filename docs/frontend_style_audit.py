"""Audit fixed UI spacing, type weights and functional icons. No application data is logged."""
from pathlib import Path
import re

async def assert_computed_styles(page):
    failures = await page.evaluate("""() => {
      const failures = [];
      for (const e of document.querySelectorAll('body *')) {
        if (!e.getClientRects().length || e.closest('svg') && !e.matches('.lucide')) continue;
        const s = getComputedStyle(e);
        const name = e.tagName + '.' + [...e.classList].slice(0, 2).join('.');
        for (const property of ['paddingTop', 'paddingRight', 'paddingBottom', 'paddingLeft', 'columnGap', 'rowGap']) {
          const value = s[property];
          if (value.endsWith('px') && Math.abs(parseFloat(value) % 4) > 0.01) failures.push([name, property, value]);
        }
        if (!['400', '600'].includes(s.fontWeight)) failures.push([name, 'fontWeight', s.fontWeight]);
        if (e.matches('.lucide') && (!['16px', '20px', '24px'].includes(s.width) || s.width !== s.height || s.strokeWidth !== '1.5px')) failures.push([name, 'icon', s.width, s.height, s.strokeWidth]);
        if (s.backdropFilter !== 'none') failures.push([name, 'backdropFilter', s.backdropFilter]);
      }
      return [...new Map(failures.map(value => [JSON.stringify(value), value])).values()];
    }""")
    assert not failures, failures


def audit_sources():
    root = Path(__file__).resolve().parent.parent / 'src/web/src'
    failures = []
    count = 0
    for path in root.rglob('*'):
        if path.suffix not in {'.css', '.tsx'} or '.test.' in path.name:
            continue
        count += 1
        source = path.read_text()
        if path.suffix == '.css':
            for match in re.finditer(r'(?:padding|margin|gap|row-gap|column-gap)(?:-[\w]+)?\s*:\s*([^;]+);', source):
                for value, unit in re.findall(r'(-?[\d.]+)(px|rem|%)', match[1]):
                    pixels = float(value) * (16 if unit == 'rem' else 1)
                    if unit == '%' or pixels % 4:
                        failures.append((str(path.relative_to(root)), source[:match.start()].count('\n') + 1, match[0]))
            for match in re.finditer(r'font-weight:\s*(\d+)', source):
                if match[1] not in {'400', '600'}:
                    failures.append((str(path.relative_to(root)), match[0]))
        forbidden = r'(?<![\w-])(?:p[xytrblse]?|m[xytrblse]?|gap(?:-[xy])?|space-[xy])-(?:\d+\.\d+|px)|font-(?:medium|bold|extrabold|black|light|thin)|backdrop-blur|shadow-(?:sm|md|lg|xl|2xl|soft)'
        for match in re.finditer(forbidden, source):
            failures.append((str(path.relative_to(root)), match[0]))
    assert not failures, failures
    print(f'PASS: {count} frontend source files; fixed spacing uses 4px multiples, explicit weights 400/600, no forbidden surface shadow/blur utilities.')

if __name__ == '__main__':
    audit_sources()
