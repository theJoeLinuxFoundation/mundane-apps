//! UDisks inventory and user-reviewed storage operations.
use anyhow::{Context, Result, bail, ensure};
use std::{collections::HashMap, ffi::OsString, os::unix::ffi::OsStringExt, path::PathBuf};
use zbus::{
    blocking::{Connection, Proxy, connection::Builder},
    fdo::ManagedObjects,
    proxy::MethodFlags,
    zvariant::{ObjectPath, OwnedValue, Value},
};

const NAME: &str = "org.freedesktop.UDisks2";
const ROOT: &str = "/org/freedesktop/UDisks2";
const DRIVE: &str = "org.freedesktop.UDisks2.Drive";
const BLOCK: &str = "org.freedesktop.UDisks2.Block";
const PART: &str = "org.freedesktop.UDisks2.Partition";
const ENCRYPTED: &str = "org.freedesktop.UDisks2.Encrypted";
const TABLE: &str = "org.freedesktop.UDisks2.PartitionTable";
const FS: &str = "org.freedesktop.UDisks2.Filesystem";
type Props = HashMap<String, OwnedValue>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Volume {
    pub path: String,
    pub mount_points: Vec<PathBuf>,
    owner: String,
    drive: String,
    drive_id: String,
    detected: u64,
    device_number: u64,
    uuid: String,
    partition_uuid: String,
}
impl Volume {
    pub fn same_device(&self, other: &Self) -> bool {
        self.path == other.path
            && self.owner == other.owner
            && self.drive == other.drive
            && self.drive_id == other.drive_id
            && self.detected == other.detected
            && self.device_number == other.device_number
            && self.uuid == other.uuid
            && self.partition_uuid == other.partition_uuid
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    PowerOff,
    Eject,
    Unlock,
}
impl Operation {
    pub fn label(self) -> &'static str {
        match self {
            Self::PowerOff => "Power off",
            Self::Eject => "Eject",
            Self::Unlock => "Unlock",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceAction {
    pub operation: Operation,
    path: String,
    owner: String,
    id: String,
    detected: u64,
    device_number: u64,
    sibling: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Region {
    pub label: String,
    pub bytes: u64,
    pub free: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub path: String,
    pub label: String,
    pub detail: String,
    pub size: u64,
    pub depth: usize,
    pub volume: Option<Volume>,
    pub actions: Vec<DeviceAction>,
    pub regions: Vec<Region>,
    unlocked: bool,
    parent: Option<String>,
    offset: u64,
}
#[derive(Clone, Default)]
pub struct UDisks {
    address: Option<String>,
}
impl UDisks {
    pub fn system() -> Self {
        Self::default()
    }
    pub fn at_address(address: String) -> Self {
        Self {
            address: Some(address),
        }
    }
    fn connect(&self) -> Result<Connection> {
        Ok(match &self.address {
            Some(a) => Builder::address(a.as_str())?.build()?,
            None => Connection::system()?,
        })
    }
    pub fn list(&self) -> Result<Vec<Item>> {
        inventory(&self.connect()?)
    }
    pub fn perform(&self, target: &DeviceAction, passphrase: &str) -> Result<String> {
        let conn = self.connect()?;
        let items = inventory(&conn)?;
        ensure!(
            items.iter().any(|n| n.actions.contains(target)),
            "The selected device or its capabilities changed."
        );
        let opts: HashMap<&str, Value<'_>> = HashMap::new();
        let flags = MethodFlags::AllowInteractiveAuth.into();
        if target.operation == Operation::Unlock {
            ensure!(!passphrase.is_empty(), "Enter a passphrase.");
            let proxy = Proxy::new(
                &conn,
                target.owner.as_str(),
                target.path.as_str(),
                ENCRYPTED,
            )?;
            let result = proxy.call_with_flags::<_, _, zbus::zvariant::OwnedObjectPath>(
                "Unlock",
                flags,
                &(passphrase, opts),
            );
            return operation_result(result.map(|v| v.map(|_| "Volume unlocked.".into())));
        }
        for (i, item) in items.iter().enumerate() {
            let affected = item.path == target.path
                || (target.operation == Operation::PowerOff
                    && !target.sibling.is_empty()
                    && item.actions.iter().any(|a| a.sibling == target.sibling));
            if affected {
                let busy = items[i + 1..]
                    .iter()
                    .take_while(|n| n.depth > item.depth)
                    .any(|n| {
                        n.unlocked
                            || n.volume
                                .as_ref()
                                .is_some_and(|v| !v.mount_points.is_empty())
                    });
                ensure!(
                    !busy,
                    "A volume on this device is mounted or unlocked. Release it first."
                );
            }
        }
        let proxy = Proxy::new(&conn, target.owner.as_str(), target.path.as_str(), DRIVE)?;
        let (method, message) = match target.operation {
            Operation::PowerOff => ("PowerOff", "Drive powered off."),
            Operation::Eject => ("Eject", "Media ejected."),
            _ => unreachable!(),
        };
        operation_result(
            proxy
                .call_with_flags::<_, _, ()>(method, flags, &(opts,))
                .map(|v| v.map(|()| message.into())),
        )
    }
    pub fn change(&self, selected: &Volume) -> Result<String> {
        let conn = self.connect()?;
        let current = inventory(&conn)?
            .into_iter()
            .find(|n| n.path == selected.path)
            .and_then(|n| n.volume)
            .context("The selected volume is no longer available.")?;
        ensure!(
            &current == selected,
            "The selected volume or its mount state changed. Select it again."
        );
        // Bind to the replying daemon's unique name so a replacement service
        // cannot receive a change requested against the previous inventory.
        let proxy = Proxy::new(&conn, selected.owner.as_str(), selected.path.as_str(), FS)?;
        let opts: HashMap<&str, Value<'_>> = HashMap::new();
        let flags = MethodFlags::AllowInteractiveAuth.into();
        let result = if selected.mount_points.is_empty() {
            proxy
                .call_with_flags::<_, _, String>("Mount", flags, &(opts,))
                .map(|p| p.map(|s| format!("Mounted at {s}.")))
        } else {
            proxy
                .call_with_flags::<_, _, ()>("Unmount", flags, &(opts,))
                .map(|r| r.map(|()| "Volume unmounted.".into()))
        };
        operation_result(result)
    }
}
fn operation_result(result: zbus::Result<Option<String>>) -> Result<String> {
    match result {
        Ok(Some(msg)) => Ok(msg),
        Ok(None) => bail!("No UDisks reply."),
        Err(zbus::Error::MethodError(name, detail, _)) => {
            let message = match name.as_str() {
                "org.freedesktop.UDisks2.Error.NotAuthorizedDismissed" => {
                    "Authorization dismissed."
                }
                "org.freedesktop.UDisks2.Error.NotAuthorized"
                | "org.freedesktop.UDisks2.Error.NotAuthorizedCanObtain" => "Authorization denied.",
                "org.freedesktop.DBus.Error.UnknownObject"
                | "org.freedesktop.DBus.Error.UnknownInterface" => {
                    "The selected volume is no longer available."
                }
                "org.freedesktop.UDisks2.Error.DeviceBusy" => "The volume is busy.",
                _ => "UDisks could not complete the operation.",
            };
            bail!("{message} {}", detail.as_deref().unwrap_or(""))
        }
        Err(e) => Err(e.into()),
    }
}
fn string(p: &Props, key: &str) -> String {
    p.get(key)
        .and_then(|v| <&str>::try_from(v).ok())
        .unwrap_or("")
        .to_owned()
}
fn number(p: &Props, key: &str) -> u64 {
    p.get(key)
        .and_then(|v| u64::try_from(v).ok())
        .unwrap_or_default()
}
fn object(p: &Props, key: &str) -> Option<String> {
    let s = <&ObjectPath>::try_from(p.get(key)?).ok()?.as_str();
    (s != "/").then(|| s.to_string())
}
fn path_bytes(mut b: Vec<u8>) -> PathBuf {
    if b.last() == Some(&0) {
        b.pop();
    }
    OsString::from_vec(b).into()
}
fn inventory(conn: &Connection) -> Result<Vec<Item>> {
    let reply = conn.call_method(
        Some(NAME),
        ROOT,
        Some("org.freedesktop.DBus.ObjectManager"),
        "GetManagedObjects",
        &(),
    )?;
    let owner = reply
        .header()
        .sender()
        .context("Missing UDisks owner")?
        .to_string();
    let objects: ManagedObjects = reply.body().deserialize()?;
    let mut items = Vec::new();
    for (path, ifaces) in &objects {
        if let Some(p) = ifaces.get(DRIVE) {
            let vendor = string(p, "Vendor");
            let model = string(p, "Model");
            let name = format!("{} {}", vendor.trim(), model.trim())
                .trim()
                .to_owned();
            let bus = string(p, "ConnectionBus");
            let kind = if p
                .get("Removable")
                .and_then(|v| bool::try_from(v).ok())
                .unwrap_or(false)
            {
                "Removable drive"
            } else {
                "Drive"
            };
            items.push(Item {
                path: path.to_string(),
                label: if name.is_empty() {
                    path.as_str().rsplit('/').next().unwrap().into()
                } else {
                    name
                },
                detail: if bus.is_empty() {
                    kind.into()
                } else {
                    format!("{kind} · {}", bus.to_uppercase())
                },
                size: number(p, "Size"),
                depth: 0,
                parent: None,
                offset: 0,
                volume: None,
                unlocked: false,
                regions: vec![],
                actions: [
                    ("CanPowerOff", Operation::PowerOff),
                    ("Ejectable", Operation::Eject),
                ]
                .into_iter()
                .filter(|(key, _)| p.get(*key).and_then(|v| bool::try_from(v).ok()) == Some(true))
                .map(|(_, operation)| DeviceAction {
                    operation,
                    path: path.to_string(),
                    owner: owner.clone(),
                    id: string(p, "Id"),
                    detected: number(p, "TimeDetected").max(number(p, "TimeMediaDetected")),
                    device_number: 0,
                    sibling: string(p, "SiblingId"),
                })
                .collect(),
            });
            continue;
        }
        let Some(p) = ifaces.get(BLOCK) else { continue };
        if p.get("HintIgnore").and_then(|v| bool::try_from(v).ok()) == Some(true) {
            continue;
        }
        let part = ifaces.get(PART);
        let drive = object(p, "Drive");
        let drive_props = p
            .get("Drive")
            .and_then(|v| <&ObjectPath>::try_from(v).ok())
            .and_then(|d| objects.get(d))
            .and_then(|i| i.get(DRIVE));
        let device = path_bytes(Vec::<u8>::try_from(
            p.get("PreferredDevice")
                .or_else(|| p.get("Device"))
                .context("Missing block device name")?
                .try_clone()?,
        )?);
        let title = string(p, "IdLabel");
        let partition = part
            .and_then(|p| p.get("Number"))
            .and_then(|v| u32::try_from(v).ok());
        let label = if !title.is_empty() {
            title
        } else if let Some(n) = partition {
            format!("Partition {n}")
        } else {
            device.display().to_string()
        };
        let mut detail = Vec::new();
        if label != device.to_string_lossy() {
            detail.push(device.display().to_string());
        }
        if let Some(n) = partition
            && label != format!("Partition {n}")
        {
            detail.push(format!("Partition {n}"));
        }
        let fstype = string(p, "IdType");
        if !fstype.is_empty() {
            detail.push(fstype)
        }
        let volume = if let Some(fs) = ifaces.get(FS) {
            let mounts = Vec::<Vec<u8>>::try_from(
                fs.get("MountPoints")
                    .context("Missing UDisks mount state")?
                    .try_clone()?,
            )?
            .into_iter()
            .map(path_bytes)
            .collect::<Vec<_>>();
            if mounts.is_empty() {
                detail.push("Unmounted".into())
            } else {
                for path in &mounts {
                    detail.push(format!("Mounted at {}", path.display()))
                }
            }
            Some(Volume {
                path: path.to_string(),
                owner: owner.clone(),
                mount_points: mounts,
                drive: drive.clone().unwrap_or_default(),
                drive_id: drive_props.map(|p| string(p, "Id")).unwrap_or_default(),
                detected: drive_props
                    .map(|p| number(p, "TimeMediaDetected").max(number(p, "TimeDetected")))
                    .unwrap_or(0),
                uuid: string(p, "IdUUID"),
                partition_uuid: part.map(|p| string(p, "UUID")).unwrap_or_default(),
                device_number: u64::try_from(
                    p.get("DeviceNumber")
                        .context("Missing block device number")?,
                )?,
            })
        } else {
            None
        };
        let crypto = ifaces.get(ENCRYPTED);
        let unlocked = crypto.is_some_and(|p| object(p, "CleartextDevice").is_some());
        let mut actions = vec![];
        if crypto.is_some() {
            detail.push(if unlocked { "Unlocked" } else { "Locked" }.into());
            if !unlocked {
                actions.push(DeviceAction {
                    operation: Operation::Unlock,
                    path: path.to_string(),
                    owner: owner.clone(),
                    id: format!(
                        "{}:{}",
                        string(p, "IdUUID"),
                        part.map(|p| string(p, "UUID")).unwrap_or_default()
                    ),
                    detected: drive_props
                        .map(|p| number(p, "TimeMediaDetected").max(number(p, "TimeDetected")))
                        .unwrap_or(0),
                    device_number: number(p, "DeviceNumber"),
                    sibling: String::new(),
                });
            }
        }
        let mut regions = vec![];
        if let Some(table) = ifaces.get(TABLE) {
            let mut spans = Vec::new();
            for (_, interfaces) in &objects {
                if let Some(part) = interfaces.get(PART)
                    && object(part, "Table").as_deref() == Some(path.as_str())
                    && !part
                        .get("IsContainer")
                        .and_then(|v| bool::try_from(v).ok())
                        .unwrap_or(false)
                    && let Some(block) = interfaces.get(BLOCK)
                {
                    let start = number(part, "Offset");
                    let size = part
                        .get("Size")
                        .and_then(|v| u64::try_from(v).ok())
                        .unwrap_or_else(|| number(block, "Size"));
                    let kind = string(block, "IdType");
                    spans.push((
                        start,
                        size,
                        if kind.is_empty() {
                            "Partition".into()
                        } else {
                            kind
                        },
                    ));
                }
            }
            regions = partition_regions(number(p, "Size"), spans);
            detail.push(format!(
                "{} partition table",
                string(table, "Type").to_uppercase()
            ))
        }
        items.push(Item {
            path: path.to_string(),
            parent: part
                .and_then(|p| object(p, "Table"))
                .or_else(|| object(p, "CryptoBackingDevice"))
                .or(drive),
            label,
            detail: detail.join(" · "),
            size: number(p, "Size"),
            depth: 0,
            volume,
            actions,
            unlocked,
            regions,
            offset: part.map(|p| number(p, "Offset")).unwrap_or(0),
        })
    }
    // Drives contain block devices and partition tables contain partitions.
    // Also retain drive-less volumes, e.g. a loop device or a mapped volume.
    items.sort_by(|a, b| {
        a.offset
            .cmp(&b.offset)
            .then(a.label.cmp(&b.label))
            .then(a.path.cmp(&b.path))
    });
    let indices: HashMap<_, _> = items
        .iter()
        .enumerate()
        .map(|(i, n)| (n.path.as_str(), i))
        .collect();
    let mut children = vec![Vec::new(); items.len()];
    let mut roots = Vec::new();
    for (i, n) in items.iter().enumerate() {
        match n.parent.as_ref().and_then(|p| indices.get(p.as_str())) {
            Some(&parent) => children[parent].push(i),
            None => roots.push(i),
        }
    }
    let mut result = Vec::new();
    let mut pending: Vec<_> = roots.into_iter().rev().map(|i| (i, 0)).collect();
    while let Some((i, depth)) = pending.pop() {
        let mut item = items[i].clone();
        item.depth = depth;
        result.push(item);
        pending.extend(children[i].iter().rev().map(|&i| (i, depth + 1)));
    }
    ensure!(
        result.len() == items.len(),
        "Inconsistent UDisks device hierarchy."
    );
    Ok(result)
}

fn partition_regions(size: u64, mut spans: Vec<(u64, u64, String)>) -> Vec<Region> {
    spans.sort();
    let mut regions = vec![];
    let mut cursor = 0;
    for (offset, len, label) in spans {
        let start = offset.min(size);
        let end = offset.saturating_add(len).min(size);
        if start > cursor {
            regions.push(Region {
                label: "Unpartitioned".into(),
                bytes: start - cursor,
                free: true,
            });
        }
        let left = cursor.max(start);
        if end > left {
            regions.push(Region {
                label,
                bytes: end - left,
                free: false,
            });
        }
        cursor = cursor.max(end);
    }
    if size > cursor {
        regions.push(Region {
            label: "Unpartitioned".into(),
            bytes: size - cursor,
            free: true,
        });
    }
    regions
}
