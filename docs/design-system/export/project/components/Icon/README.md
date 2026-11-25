# Icon

Vector icon from the built-in Lucide set.

**Family:** Media · **Rust:** `ravenrs::prelude::Icon`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `size` | number 0–96 by 1 | `24` |  |

## Stories

- **Example**: 96 × 29 pt, settings `{"size":24.0}`
- **Large**: 120 × 58 pt, settings `{"size":48.0}`

## Sizing

Hugs its content: 96 × 29 pt in a parent of 760 pt.

## API

Builder methods: `color`, `new`, `size`.

`IconName`: `Activity`, `AlarmClock`, `AlignCenter`, `AlignJustify`, `AlignLeft`, `AlignRight`, `ArrowDown`, `ArrowDownWideNarrow`, `ArrowLeft`, `ArrowRight`, `ArrowUp`, `ArrowUpDown`, `ArrowUpRight`, `Award`, `Baseline`, `Battery`, `Bell`, `BellRing`, `Bluetooth`, `Bold`, `BookOpen`, `Bookmark`, `Bot`, `Brain`, `Calculator`, `Calendar`, `CalendarCheck`, `CalendarDays`, `Camera`, `ChartBar`, `ChartColumn`, `ChartLine`, `ChartPie`, `Check`, `ChevronDown`, `ChevronLeft`, `ChevronRight`, `ChevronUp`, `ChevronsUpDown`, `Circle`, `CircleAlert`, `CircleCheck`, `CircleHelp`, `CircleX`, `Clipboard`, `ClipboardPaste`, `Clock`, `Clock3`, `Cloud`, `CloudDownload`, `CloudUpload`, `Code`, `Columns3`, `Command`, `Compass`, `Copy`, `Cpu`, `CreditCard`, `Crosshair`, `Database`, `DollarSign`, `Download`, `Ellipsis`, `EllipsisVertical`, `ExternalLink`, `Eye`, `EyeClosed`, `EyeOff`, `File`, `FilePlus`, `FileText`, `Filter`, `FilterX`, `Flag`, `FlagTriangleRight`, `Folder`, `FolderOpen`, `Gift`, `GitBranch`, `Globe`, `Grid2x2`, `Grid3x3`, `HardDrive`, `Heading`, `Headphones`, `Heart`, `Highlighter`, `History`, `House`, `Image`, `ImagePlus`, `Inbox`, `Info`, `Italic`, `Key`, `Keyboard`, `Languages`, `Layers`, `LayoutDashboard`, `Link`, `Link2`, `List`, `ListOrdered`, `LoaderCircle`, `Lock`, `LockKeyhole`, `LockOpen`, `LogIn`, `LogOut`, `Mail`, `Map`, `MapPin`, `Maximize2`, `Menu`, `Merge`, `MessageCircle`, `MessageSquare`, `Mic`, `Minimize2`, `Minus`, `Monitor`, `Moon`, `MousePointer`, `Music`, `Navigation`, `Package`, `Paintbrush`, `Palette`, `PanelLeft`, `Pause`, `Pencil`, `Phone`, `Pilcrow`, `Pin`, `PinOff`, `Play`, `Plug`, `Plus`, `PlusCircle`, `Power`, `Printer`, `Quote`, `Redo2`, `RefreshCcw`, `RefreshCw`, `Rocket`, `RotateCcw`, `Route`, `Rows3`, `Ruler`, `Save`, `Scissors`, `Search`, `Send`, `Server`, `Settings`, `Shapes`, `Share`, `Share2`, `Shield`, `ShoppingCart`, `Sigma`, `SlidersHorizontal`, `Smartphone`, `Smile`, `Sparkles`, `SpellCheck`, `Split`, `Square`, `SquareFunction`, `Star`, `StarOff`, `Strikethrough`, `Sun`, `Table`, `Table2`, `Tag`, `Target`, `Terminal`, `TextCursor`, `ThumbsDown`, `ThumbsUp`, `Timer`, `Trash2`, `TrendingDown`, `TrendingUp`, `TriangleAlert`, `Trophy`, `Type`, `Underline`, `Undo2`, `Upload`, `User`, `UserCheck`, `UserPlus`, `Users`, `Video`, `Volume2`, `VolumeX`, `Wallet`, `Wifi`, `WifiOff`, `Workflow`, `X`, `Zap`, `ZoomIn`, `ZoomOut`.

## Example

```rust
fn icon() -> impl View {
    HStack::new((
        Icon::new(IconName::Sparkles).size(24.0).color(Semantic::Accent),
        Icon::new(IconName::Bell).size(24.0),
        Icon::new(IconName::Check).size(24.0).color(Semantic::Success),
    ))
    .spacing(12.0)
}
```

## Every icon

Each icon is one character of the `RavenIcons` font (in `fonts/`): write it as `&#x…;` in a span whose `font-family` is `RavenIcons`, sized like text.

| Name | `IconName` | Character |
|---|---|---|
| `search` | `Search` | `&#xe151;` |
| `x` | `X` | `&#xe1b2;` |
| `check` | `Check` | `&#xe06c;` |
| `plus` | `Plus` | `&#xe13d;` |
| `minus` | `Minus` | `&#xe11c;` |
| `chevron-down` | `ChevronDown` | `&#xe06d;` |
| `chevron-up` | `ChevronUp` | `&#xe070;` |
| `chevron-left` | `ChevronLeft` | `&#xe06e;` |
| `chevron-right` | `ChevronRight` | `&#xe06f;` |
| `arrow-up` | `ArrowUp` | `&#xe04a;` |
| `arrow-down` | `ArrowDown` | `&#xe042;` |
| `arrow-left` | `ArrowLeft` | `&#xe048;` |
| `arrow-right` | `ArrowRight` | `&#xe049;` |
| `arrow-up-right` | `ArrowUpRight` | `&#xe04d;` |
| `external-link` | `ExternalLink` | `&#xe0b9;` |
| `link` | `Link` | `&#xe102;` |
| `house` | `House` | `&#xe0f5;` |
| `layout-dashboard` | `LayoutDashboard` | `&#xe1c1;` |
| `panel-left` | `PanelLeft` | `&#xe12a;` |
| `menu` | `Menu` | `&#xe115;` |
| `ellipsis` | `Ellipsis` | `&#xe0b6;` |
| `ellipsis-vertical` | `EllipsisVertical` | `&#xe0b7;` |
| `grid-3x3` | `Grid3x3` | `&#xe0e9;` |
| `list` | `List` | `&#xe106;` |
| `table` | `Table` | `&#xe17d;` |
| `settings` | `Settings` | `&#xe154;` |
| `sliders-horizontal` | `SlidersHorizontal` | `&#xe29a;` |
| `filter` | `Filter` | `&#xe0dc;` |
| `user` | `User` | `&#xe19f;` |
| `users` | `Users` | `&#xe1a4;` |
| `bell` | `Bell` | `&#xe059;` |
| `mail` | `Mail` | `&#xe10f;` |
| `message-square` | `MessageSquare` | `&#xe117;` |
| `send` | `Send` | `&#xe152;` |
| `inbox` | `Inbox` | `&#xe0f7;` |
| `calendar` | `Calendar` | `&#xe063;` |
| `clock` | `Clock` | `&#xe087;` |
| `star` | `Star` | `&#xe176;` |
| `heart` | `Heart` | `&#xe0f2;` |
| `bookmark` | `Bookmark` | `&#xe060;` |
| `tag` | `Tag` | `&#xe17f;` |
| `flag` | `Flag` | `&#xe0d1;` |
| `award` | `Award` | `&#xe04f;` |
| `shield` | `Shield` | `&#xe158;` |
| `key` | `Key` | `&#xe0fd;` |
| `lock` | `Lock` | `&#xe10b;` |
| `lock-open` | `LockOpen` | `&#xe10c;` |
| `trash-2` | `Trash2` | `&#xe18e;` |
| `pencil` | `Pencil` | `&#xe1f9;` |
| `copy` | `Copy` | `&#xe09e;` |
| `clipboard` | `Clipboard` | `&#xe085;` |
| `download` | `Download` | `&#xe0b2;` |
| `upload` | `Upload` | `&#xe19e;` |
| `share-2` | `Share2` | `&#xe156;` |
| `refresh-cw` | `RefreshCw` | `&#xe145;` |
| `rotate-ccw` | `RotateCcw` | `&#xe148;` |
| `eye` | `Eye` | `&#xe0ba;` |
| `eye-off` | `EyeOff` | `&#xe0bb;` |
| `log-out` | `LogOut` | `&#xe10e;` |
| `log-in` | `LogIn` | `&#xe10d;` |
| `plus-circle` | `PlusCircle` | `&#xe081;` |
| `circle-check` | `CircleCheck` | `&#xe226;` |
| `circle-alert` | `CircleAlert` | `&#xe077;` |
| `triangle-alert` | `TriangleAlert` | `&#xe193;` |
| `info` | `Info` | `&#xe0f9;` |
| `circle-help` | `CircleHelp` | `&#xe082;` |
| `circle-x` | `CircleX` | `&#xe084;` |
| `loader-circle` | `LoaderCircle` | `&#xe10a;` |
| `sparkles` | `Sparkles` | `&#xe412;` |
| `zap` | `Zap` | `&#xe1b4;` |
| `rocket` | `Rocket` | `&#xe286;` |
| `command` | `Command` | `&#xe09a;` |
| `keyboard` | `Keyboard` | `&#xe284;` |
| `monitor` | `Monitor` | `&#xe11d;` |
| `smartphone` | `Smartphone` | `&#xe163;` |
| `sun` | `Sun` | `&#xe178;` |
| `moon` | `Moon` | `&#xe11e;` |
| `palette` | `Palette` | `&#xe1dd;` |
| `layers` | `Layers` | `&#xe529;` |
| `folder` | `Folder` | `&#xe0d7;` |
| `file` | `File` | `&#xe0c0;` |
| `file-text` | `FileText` | `&#xe0cc;` |
| `image` | `Image` | `&#xe0f6;` |
| `code` | `Code` | `&#xe093;` |
| `terminal` | `Terminal` | `&#xe181;` |
| `git-branch` | `GitBranch` | `&#xe0e2;` |
| `globe` | `Globe` | `&#xe0e8;` |
| `cloud` | `Cloud` | `&#xe088;` |
| `database` | `Database` | `&#xe0ad;` |
| `server` | `Server` | `&#xe153;` |
| `cpu` | `Cpu` | `&#xe0a9;` |
| `activity` | `Activity` | `&#xe038;` |
| `trending-up` | `TrendingUp` | `&#xe191;` |
| `trending-down` | `TrendingDown` | `&#xe190;` |
| `chart-bar` | `ChartBar` | `&#xe2a2;` |
| `chart-line` | `ChartLine` | `&#xe2a5;` |
| `chart-pie` | `ChartPie` | `&#xe06b;` |
| `dollar-sign` | `DollarSign` | `&#xe0b1;` |
| `credit-card` | `CreditCard` | `&#xe0aa;` |
| `wallet` | `Wallet` | `&#xe204;` |
| `shopping-cart` | `ShoppingCart` | `&#xe15c;` |
| `package` | `Package` | `&#xe129;` |
| `map-pin` | `MapPin` | `&#xe111;` |
| `wifi` | `Wifi` | `&#xe1ae;` |
| `battery` | `Battery` | `&#xe053;` |
| `play` | `Play` | `&#xe13c;` |
| `pause` | `Pause` | `&#xe12e;` |
| `square` | `Square` | `&#xe167;` |
| `circle` | `Circle` | `&#xe076;` |
| `maximize-2` | `Maximize2` | `&#xe113;` |
| `minimize-2` | `Minimize2` | `&#xe11b;` |
| `mouse-pointer` | `MousePointer` | `&#xe11f;` |
| `scissors` | `Scissors` | `&#xe14e;` |
| `clipboard-paste` | `ClipboardPaste` | `&#xe3e8;` |
| `bold` | `Bold` | `&#xe05d;` |
| `italic` | `Italic` | `&#xe0fb;` |
| `underline` | `Underline` | `&#xe19a;` |
| `strikethrough` | `Strikethrough` | `&#xe177;` |
| `align-left` | `AlignLeft` | `&#xe185;` |
| `align-center` | `AlignCenter` | `&#xe182;` |
| `align-right` | `AlignRight` | `&#xe183;` |
| `align-justify` | `AlignJustify` | `&#xe184;` |
| `list-ordered` | `ListOrdered` | `&#xe1d1;` |
| `undo-2` | `Undo2` | `&#xe2a1;` |
| `redo-2` | `Redo2` | `&#xe2a0;` |
| `save` | `Save` | `&#xe14d;` |
| `printer` | `Printer` | `&#xe141;` |
| `type` | `Type` | `&#xe198;` |
| `highlighter` | `Highlighter` | `&#xe0f4;` |
| `paintbrush` | `Paintbrush` | `&#xe2e7;` |
| `baseline` | `Baseline` | `&#xe285;` |
| `heading` | `Heading` | `&#xe384;` |
| `quote` | `Quote` | `&#xe239;` |
| `zoom-in` | `ZoomIn` | `&#xe1b6;` |
| `zoom-out` | `ZoomOut` | `&#xe1b7;` |
| `file-plus` | `FilePlus` | `&#xe0c9;` |
| `folder-open` | `FolderOpen` | `&#xe247;` |
| `link-2` | `Link2` | `&#xe103;` |
| `table-2` | `Table2` | `&#xe2f9;` |
| `image-plus` | `ImagePlus` | `&#xe1f7;` |
| `chart-column` | `ChartColumn` | `&#xe2a3;` |
| `shapes` | `Shapes` | `&#xe4b3;` |
| `sigma` | `Sigma` | `&#xe201;` |
| `calculator` | `Calculator` | `&#xe1bc;` |
| `filter-x` | `FilterX` | `&#xe3b5;` |
| `arrow-up-down` | `ArrowUpDown` | `&#xe37d;` |
| `arrow-down-wide-narrow` | `ArrowDownWideNarrow` | `&#xe047;` |
| `ruler` | `Ruler` | `&#xe14b;` |
| `grid-2x2` | `Grid2x2` | `&#xe4ff;` |
| `rows-3` | `Rows3` | `&#xe58a;` |
| `columns-3` | `Columns3` | `&#xe099;` |
| `merge` | `Merge` | `&#xe43f;` |
| `split` | `Split` | `&#xe440;` |
| `pilcrow` | `Pilcrow` | `&#xe3a3;` |
| `text-cursor` | `TextCursor` | `&#xe264;` |
| `languages` | `Languages` | `&#xe0fe;` |
| `spell-check` | `SpellCheck` | `&#xe49a;` |
| `message-circle` | `MessageCircle` | `&#xe116;` |
| `share` | `Share` | `&#xe155;` |
| `history` | `History` | `&#xe1f5;` |
| `book-open` | `BookOpen` | `&#xe05f;` |
| `eye-closed` | `EyeClosed` | `&#xe62e;` |
| `lock-keyhole` | `LockKeyhole` | `&#xe531;` |
| `user-plus` | `UserPlus` | `&#xe1a2;` |
| `user-check` | `UserCheck` | `&#xe1a0;` |
| `bell-ring` | `BellRing` | `&#xe224;` |
| `calendar-days` | `CalendarDays` | `&#xe2b9;` |
| `clock-3` | `Clock3` | `&#xe250;` |
| `timer` | `Timer` | `&#xe1e0;` |
| `alarm-clock` | `AlarmClock` | `&#xe03a;` |
| `map` | `Map` | `&#xe110;` |
| `compass` | `Compass` | `&#xe09b;` |
| `navigation` | `Navigation` | `&#xe123;` |
| `phone` | `Phone` | `&#xe133;` |
| `video` | `Video` | `&#xe1a5;` |
| `camera` | `Camera` | `&#xe064;` |
| `mic` | `Mic` | `&#xe118;` |
| `headphones` | `Headphones` | `&#xe0f1;` |
| `music` | `Music` | `&#xe122;` |
| `volume-2` | `Volume2` | `&#xe1ab;` |
| `volume-x` | `VolumeX` | `&#xe1ac;` |
| `cloud-upload` | `CloudUpload` | `&#xe091;` |
| `cloud-download` | `CloudDownload` | `&#xe089;` |
| `hard-drive` | `HardDrive` | `&#xe0ed;` |
| `wifi-off` | `WifiOff` | `&#xe1af;` |
| `bluetooth` | `Bluetooth` | `&#xe05c;` |
| `plug` | `Plug` | `&#xe37f;` |
| `power` | `Power` | `&#xe140;` |
| `refresh-ccw` | `RefreshCcw` | `&#xe144;` |
| `thumbs-up` | `ThumbsUp` | `&#xe18a;` |
| `thumbs-down` | `ThumbsDown` | `&#xe189;` |
| `smile` | `Smile` | `&#xe164;` |
| `star-off` | `StarOff` | `&#xe2b0;` |
| `pin` | `Pin` | `&#xe259;` |
| `pin-off` | `PinOff` | `&#xe2b6;` |
| `gift` | `Gift` | `&#xe0e1;` |
| `trophy` | `Trophy` | `&#xe373;` |
| `target` | `Target` | `&#xe180;` |
| `crosshair` | `Crosshair` | `&#xe0ac;` |
| `flag-triangle-right` | `FlagTriangleRight` | `&#xe238;` |
| `square-function` | `SquareFunction` | `&#xe22d;` |
| `brain` | `Brain` | `&#xe3c6;` |
| `route` | `Route` | `&#xe53e;` |
| `bot` | `Bot` | `&#xe1bb;` |
| `workflow` | `Workflow` | `&#xe425;` |
| `calendar-check` | `CalendarCheck` | `&#xe2b7;` |
| `chevrons-up-down` | `ChevronsUpDown` | `&#xe211;` |

Every number above is in `Icon.json` beside this card.
