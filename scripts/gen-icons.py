#!/usr/bin/env python3
"""Generates RavenRS's icon set from the Lucide icon font (ISC license).

Usage: gen-icons.py <lucide.ttf> <lucide.css>
       gen-icons.py --add <lucide.ttf> <lucide.css> <name>...
Requires fontTools (pip install fonttools). Writes:
  crates/ravenrs-text/fonts/RavenIcons.ttf   (subset with only the icons below)
  crates/ravenrs-widgets-base/src/icon_set.rs (the `IconName` enum)

Add an icon by appending its Lucide name to ICONS and running with --add: it copies only the new
glyphs into the font there is, so the others stay exactly as they are, and appends them to
icon_set.rs. Regenerating everything takes the version of Lucide each glyph came from, which the
font does not record; the versions are written next to ICONS.

The font came from a Lucide release that was not written down (199 of its 200 outlines are
the same in lucide-static 1.73.0). square-function, brain, route, bot and workflow came from
lucide-static 1.73.0.
"""
import re
import sys
from pathlib import Path

from fontTools import subset
from fontTools.ttLib import TTFont

ICONS = """
search x check plus minus chevron-down chevron-up chevron-left chevron-right
arrow-up arrow-down arrow-left arrow-right arrow-up-right external-link link
house layout-dashboard panel-left menu ellipsis ellipsis-vertical grid-3x3 list table
settings sliders-horizontal filter user users bell mail message-square send inbox
calendar clock star heart bookmark tag flag award shield key lock lock-open
trash-2 pencil copy clipboard download upload share-2 refresh-cw rotate-ccw
eye eye-off log-out log-in plus-circle circle-check circle-alert triangle-alert info
circle-help circle-x loader-circle sparkles zap rocket command keyboard monitor
smartphone sun moon palette layers folder file file-text image code terminal
git-branch globe cloud database server cpu activity trending-up trending-down
chart-bar chart-line chart-pie dollar-sign credit-card wallet shopping-cart package
map-pin wifi battery play pause square circle maximize-2 minimize-2 mouse-pointer
scissors clipboard-paste bold italic underline strikethrough align-left align-center align-right
align-justify list-ordered undo-2 redo-2 save printer type highlighter paintbrush baseline heading
quote zoom-in zoom-out file-plus folder-open link-2 table-2 image-plus chart-column shapes
sigma calculator filter-x arrow-up-down arrow-down-wide-narrow ruler grid-2x2 rows-3 columns-3
merge split pilcrow text-cursor languages spell-check message-circle share history book-open
eye-closed lock-keyhole user-plus user-check bell-ring calendar-days clock-3 timer alarm-clock
map compass navigation phone video camera mic headphones music volume-2 volume-x
cloud-upload cloud-download hard-drive cpu wifi-off bluetooth plug power refresh-ccw
thumbs-up thumbs-down smile star-off pin pin-off gift trophy target crosshair flag-triangle-right
square-function brain route bot workflow
""".split()


def codepoints_of(css: Path) -> dict:
    return {
        m.group(1): int(m.group(2), 16)
        for m in re.finditer(r'\.icon-([a-z0-9-]+)::?before\s*\{\s*content:\s*"\\([0-9a-fA-F]+)"', css.read_text())
    }


def ident(name: str) -> str:
    return "".join(part[:1].upper() + part[1:] for part in name.split("-"))


def add(ttf: Path, css: Path, names: list) -> None:
    """Copies these glyphs of Lucide into the RavenIcons font there is, and adds them to icon_set.rs."""
    root = Path(__file__).resolve().parent.parent
    out_font = root / "crates/ravenrs-text/fonts/RavenIcons.ttf"
    icon_set = root / "crates/ravenrs-widgets-base/src/icon_set.rs"
    codepoints = codepoints_of(css)
    ours, theirs = TTFont(str(out_font)), TTFont(str(ttf))
    assert ours["head"].unitsPerEm == theirs["head"].unitsPerEm, "different units per em"
    source = theirs.getBestCmap()
    text = icon_set.read_text()
    # Every name is checked before anything is written, so a wrong one leaves the font whole.
    unknown = [name for name in names if name not in codepoints or codepoints[name] not in source]
    if unknown:
        sys.exit(f"not in this Lucide font, nothing written: {', '.join(unknown)}")
    for name in names:
        if f'=> "{name}",' in text:
            print(f"{name}: already there")
            continue
        point = codepoints[name]
        assert point not in ours.getBestCmap(), f"{name}: U+{point:X} is taken"
        glyph_name = f"rv-{name}"
        glyph = theirs["glyf"][source[point]]
        glyph.recalcBounds(theirs["glyf"])
        advance, _ = theirs["hmtx"][source[point]]
        ours["glyf"].glyphs[glyph_name] = glyph
        ours.setGlyphOrder(ours.getGlyphOrder() + [glyph_name])
        # The bearing is the outline's left edge, as main() leaves every glyph (see there).
        ours["hmtx"][glyph_name] = (advance, glyph.xMin if glyph.numberOfContours else 0)
        for table in ours["cmap"].tables:
            if table.isUnicode():
                table.cmap[point] = glyph_name
        ident_name = ident(name)
        # The enum ends at the first line that is only a brace.
        end = text.index("\n}\n")
        text = text[:end] + f"\n    /// The Lucide `{name}` icon.\n    {ident_name}," + text[end:]
        text = text.replace("\n    ];\n", f"\n        IconName::{ident_name},\n    ];\n", 1)
        marker = "\n        }\n    }\n\n    /// The character that draws"
        text = text.replace(marker, f'\n            IconName::{ident_name} => "{name}",{marker}', 1)
        text = text.rstrip().removesuffix("}").rstrip().removesuffix("}").rstrip().removesuffix("}").rstrip()
        text += f"\n            IconName::{ident_name} => '\\u{{{point:x}}}',\n        }}\n    }}\n}}\n"
        print(f"{name}: U+{point:X}")
    ours["maxp"].numGlyphs = len(ours.getGlyphOrder())
    ours.save(str(out_font))
    icon_set.write_text(text)


def main() -> None:
    if sys.argv[1] == "--add":
        add(Path(sys.argv[2]), Path(sys.argv[3]), sys.argv[4:])
        return
    ttf, css = Path(sys.argv[1]), Path(sys.argv[2])
    root = Path(__file__).resolve().parent.parent
    codepoints = codepoints_of(css)
    missing = [n for n in ICONS if n not in codepoints]
    if missing:
        print("not in this Lucide version, skipped:", ", ".join(missing))
    # Keep the first occurrence of each name, in order.
    names = list(dict.fromkeys(n for n in ICONS if n in codepoints))

    out_font = root / "crates/ravenrs-text/fonts/RavenIcons.ttf"
    opts = subset.Options()
    opts.name_IDs = ["*"]
    opts.notdef_outline = True
    opts.layout_features = []
    font = TTFont(str(ttf))
    sub = subset.Subsetter(opts)
    sub.populate(unicodes=[codepoints[n] for n in names])
    sub.subset(font)
    # Lucide's font leaves every left side bearing at 0, while the outlines
    # start further right. Rasterizers place the outline by the bearing, so the
    # icons drew up to 2px left of center. Sync each bearing with its outline.
    glyf, hmtx = font["glyf"], font["hmtx"]
    for name in font.getGlyphOrder():
        glyph = glyf[name]
        advance, _ = hmtx[name]
        hmtx[name] = (advance, getattr(glyph, "xMin", 0) if glyph.numberOfContours else 0)
    # A distinct family name so it never collides with an installed Lucide.
    for rec in font["name"].names:
        if rec.nameID in (1, 4, 16):
            rec.string = "RavenIcons"
        elif rec.nameID == 6:
            rec.string = "RavenIcons-Regular"
    font.save(str(out_font))

    lines = [
        "// @generated by scripts/gen-icons.py from Lucide (ISC License). Do not edit.",
        "",
        "/// Icons available to [`Icon`](crate::Icon), from the Lucide set.",
        "#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]",
        "pub enum IconName {",
    ]
    for n in names:
        lines += [f"    /// The Lucide `{n}` icon.", f"    {ident(n)},"]
    lines += ["}", "", "impl IconName {", "    /// Every icon, in catalog order.", "    pub const ALL: &[IconName] = &["]
    lines += [f"        IconName::{ident(n)}," for n in names]
    lines += ["    ];", "", "    /// The Lucide name, e.g. `\"chevron-down\"`.", "    pub fn name(self) -> &'static str {", "        match self {"]
    lines += [f'            IconName::{ident(n)} => "{n}",' for n in names]
    lines += ["        }", "    }", "", "    /// The character that draws this icon in the RavenIcons font.", "    pub fn glyph(self) -> char {", "        match self {"]
    lines += [f"            IconName::{ident(n)} => '\\u{{{codepoints[n]:x}}}'," for n in names]
    lines += ["        }", "    }", "}", ""]
    (root / "crates/ravenrs-widgets-base/src/icon_set.rs").write_text("\n".join(lines))
    print(f"{len(names)} icons, font {out_font.stat().st_size} bytes")


if __name__ == "__main__":
    main()
