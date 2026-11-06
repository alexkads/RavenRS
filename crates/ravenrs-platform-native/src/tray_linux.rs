//! The icon on Linux: a StatusNotifierItem, which `ksni` speaks over D-Bus.

use ksni::blocking::TrayMethods as _;
use ksni::menu::StandardItem;
use ksni::{MenuItem, ToolTip};

use super::{ImageData, Notify, SystemTray, TrayAction};

struct Item {
    id: String,
    name: String,
    open: String,
    quit: String,
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
        vec![
            StandardItem {
                label: self.open.clone(),
                activate: Box::new(|item: &mut Self| (item.notify)(TrayAction::Open)),
                ..StandardItem::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: self.quit.clone(),
                activate: Box::new(|item: &mut Self| (item.notify)(TrayAction::Quit)),
                ..StandardItem::default()
            }
            .into(),
        ]
    }
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
    let data = icon.pixels().chunks_exact(4).flat_map(|p| [p[3], p[0], p[1], p[2]]).collect();
    let icon = ksni::Icon {
        width: i32::try_from(icon.width()).map_err(|e| e.to_string())?,
        height: i32::try_from(icon.height()).map_err(|e| e.to_string())?,
        data,
    };
    let item = Item {
        id: id.to_owned(),
        name: tray.name.clone(),
        open: tray.open.clone(),
        quit: tray.quit.clone(),
        icon,
        notify,
    };
    // Fails when nothing hosts the icon, which is the answer for plain GNOME.
    item.spawn().map(Icon).map_err(|e| e.to_string())
}
