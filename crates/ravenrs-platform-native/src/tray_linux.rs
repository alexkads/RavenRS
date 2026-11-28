//! The icon on Linux: a StatusNotifierItem, which `ksni` speaks over D-Bus.

use ksni::blocking::TrayMethods as _;
use ksni::menu::{CheckmarkItem, StandardItem, SubMenu};
use ksni::{MenuItem, ToolTip};

use super::{ImageData, Notify, Row, SystemTray, TrayAction, action_of};

struct Item {
    id: String,
    name: String,
    rows: Vec<Row>,
    icon: ksni::Icon,
    notify: Notify,
}

impl ksni::Tray for Item {
    fn id(&self) -> String {
        self.id.clone()
    }

    fn title(&self) -> String {
        self.name.clone()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![ksni::Icon { width: self.icon.width, height: self.icon.height, data: self.icon.data.clone() }]
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip { title: self.name.clone(), ..ToolTip::default() }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        (self.notify)(TrayAction::Open);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        items(&self.rows)
    }
}

/// The same rows as the other systems, with the same ids.
fn items(rows: &[Row]) -> Vec<MenuItem<Item>> {
    rows.iter()
        .map(|row| match row {
            Row::Item { id, label, enabled, .. } => {
                let id = id.clone();
                StandardItem {
                    label: label.clone(),
                    enabled: *enabled,
                    activate: Box::new(move |item: &mut Item| {
                        if let Some(action) = action_of(&id) {
                            (item.notify)(action);
                        }
                    }),
                    ..StandardItem::default()
                }
                .into()
            }
            Row::Check { id, label, checked } => {
                let id = id.clone();
                CheckmarkItem {
                    label: label.clone(),
                    checked: *checked,
                    activate: Box::new(move |item: &mut Item| {
                        if let Some(action) = action_of(&id) {
                            (item.notify)(action);
                        }
                    }),
                    ..CheckmarkItem::default()
                }
                .into()
            }
            Row::Sub { label, rows } => {
                SubMenu { label: label.clone(), submenu: items(rows), ..SubMenu::default() }.into()
            }
            Row::Separator => MenuItem::Separator,
        })
        .collect()
}

pub(super) struct Icon(ksni::blocking::Handle<Item>);

impl Drop for Icon {
    fn drop(&mut self) {
        // Asks and does not wait: the icon leaving is not worth holding the window for.
        let _ = self.0.shutdown();
    }
}

pub(super) fn show(tray: &SystemTray, id: &str, icon: &ImageData, notify: Notify) -> Result<Icon, String> {
    // RGBA to the ARGB of the protocol.
    let data = icon.pixels().as_chunks::<4>().0.iter().flat_map(|p| [p[3], p[0], p[1], p[2]]).collect();
    let icon = ksni::Icon {
        width: i32::try_from(icon.width()).map_err(|e| e.to_string())?,
        height: i32::try_from(icon.height()).map_err(|e| e.to_string())?,
        data,
    };
    let item = Item { id: id.to_owned(), name: tray.name.clone(), rows: tray.rows(), icon, notify };
    // Fails when nothing hosts the icon, which is the answer for plain GNOME.
    item.spawn().map(Icon).map_err(|e| e.to_string())
}

pub(super) fn refresh(icon: &Icon, tray: &SystemTray) -> Result<(), String> {
    let rows = tray.rows();
    icon.0.update(move |item| item.rows = rows).ok_or_else(|| "the icon is gone".to_owned())
}
